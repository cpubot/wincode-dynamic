use {
    crate::{
        PrimitiveTy,
        wincode_extra::{LenMap, Map},
    },
    core::mem::MaybeUninit,
    std::borrow::Cow,
    wincode::{
        ReadError, ReadResult, SchemaRead, SchemaReadContext,
        config::{Config, ConfigCore, DefaultConfig},
        context::Len,
        io::Reader,
        len::SeqLen,
    },
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PrimitiveValue {
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    Bool(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value<'a> {
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    Bool(bool),
    String(Cow<'a, str>),
    Bytes(Cow<'a, [u8]>),
    Vec(LazyVec<'a>),
}

#[derive(Debug, Clone, PartialEq)]
/// A lazily decoded vector of fixed-width primitive values.
///
/// The encoded payload remains borrowed when the input reader supports stable
/// borrowing. Use [`Self::try_into_iter_as`] when the concrete element type is
/// known, or [`Self::into_dyn_vec`] to decode dynamically typed values.
pub struct LazyVec<'a> {
    pub(crate) ty: PrimitiveTy,
    pub(crate) payload: Cow<'a, [u8]>,
}

/// Lazy-vector iterator types.
pub mod lazy_vec {
    use {super::*, crate::DynPrimitiveTy};

    /// An owning, lazy iterator over the elements of a [`LazyVec`].
    ///
    /// Each item is decoded only when [`Iterator::next`] is called.
    pub struct IntoIter<'a, As> {
        payload: Cow<'a, [u8]>,
        len: usize,
        index: usize,
        _marker: std::marker::PhantomData<As>,
    }

    impl<'a> LazyVec<'a> {
        /// Converts this vector into a lazy iterator of `As` values.
        ///
        /// The requested type must match the primitive element type recorded in
        /// the schema. This prevents, for example, interpreting the payload of a
        /// `Vec<u64>` as a sequence of `u8` values.
        ///
        /// This method validates the element type and payload width, but it does
        /// not decode any elements. Decoding happens as the returned iterator is
        /// advanced, and each item is returned as a [`ReadResult`]. Consequently,
        /// malformed element encodings are reported by the iterator item that
        /// encounters them rather than by this method.
        ///
        /// The lazy vector is consumed so the iterator can retain ownership of
        /// its payload. Borrowed payloads remain borrowed; no allocation or eager
        /// collection is performed by this conversion.
        ///
        /// To iterate, `As` must also implement
        /// [`SchemaRead<DefaultConfig, Dst = As>`](SchemaRead).
        ///
        /// # Errors
        ///
        /// Returns an error if:
        ///
        /// - `As` does not match the vector's recorded primitive element type; or
        /// - the payload length is not a multiple of the element width.
        ///
        /// # Examples
        ///
        /// ```
        /// use wincode::{ReadResult, SchemaRead, SchemaWrite};
        /// use wincode_dynamic::{SchemaDynamic, SchemaRuntime, Value};
        ///
        /// #[derive(SchemaDynamic, SchemaRead, SchemaWrite)]
        /// struct Message {
        ///     values: Vec<u64>,
        /// }
        ///
        /// let encoded = wincode::serialize(&Message {
        ///     values: vec![10, 20, 30],
        /// })
        /// .expect("serialize message");
        /// let runtime = SchemaRuntime::new(Message::schema());
        /// let mut fields = runtime.fields(encoded.as_slice())?;
        ///
        /// let Value::Vec(values) = fields.next().expect("values field")? else {
        ///     panic!("expected a vector");
        /// };
        /// let values = values
        ///     .try_into_iter_as::<u64>()?
        ///     .collect::<ReadResult<Vec<_>>>()?;
        ///
        /// assert_eq!(values, [10, 20, 30]);
        /// # Ok::<(), wincode::ReadError>(())
        /// ```
        #[inline]
        pub fn try_into_iter_as<As>(self) -> ReadResult<IntoIter<'a, As>>
        where
            As: DynPrimitiveTy,
        {
            #[cold]
            fn ty_mismatch() -> ReadError {
                ReadError::Custom("lazy vector element type mismatch")
            }
            if As::TYPE != self.ty {
                return Err(ty_mismatch());
            }

            let element_size = size_of::<As>();
            if element_size == 0 || !self.payload.len().is_multiple_of(element_size) {
                return Err(ReadError::Custom(
                    "lazy vector payload has an invalid length",
                ));
            }
            Ok(IntoIter {
                len: self.payload.len() / element_size,
                payload: self.payload,
                index: 0,
                _marker: std::marker::PhantomData,
            })
        }

        /// Converts this lazy vector into a [`Vec<PrimitiveValue>`] by decoding
        /// all elements using the vector's recorded primitive element type.
        ///
        /// # Errors
        ///
        /// Returns the first element decoding error encountered in the payload.
        #[inline]
        pub fn into_dyn_vec(self) -> ReadResult<Vec<PrimitiveValue>> {
            let len = self.payload.len() / self.ty.size();
            <<DefaultConfig as Config>::LengthEncoding as SeqLen<DefaultConfig>>::prealloc_check::<
                PrimitiveValue,
            >(len)?;
            <Array<PrimitiveValue> as SchemaReadContext<DefaultConfig, _>>::get_with_context(
                (Len(len), self.ty),
                &self.payload[..],
            )
        }

        /// Returns the number of elements in the vector.
        #[inline]
        pub fn len(&self) -> usize {
            self.payload.len() / self.ty.size()
        }

        /// Returns `true` if the vector contains no elements.
        #[inline]
        pub fn is_empty(&self) -> bool {
            self.payload.is_empty()
        }

        /// Returns the vector's primitive element type.
        #[inline]
        pub fn ty(&self) -> PrimitiveTy {
            self.ty
        }
    }

    impl<As> Iterator for IntoIter<'_, As>
    where
        As: for<'de> SchemaRead<'de, DefaultConfig, Dst = As>,
    {
        type Item = ReadResult<As>;

        #[inline]
        fn next(&mut self) -> Option<Self::Item> {
            if self.index >= self.len {
                return None;
            }
            let start = self.index * size_of::<As>();
            // SAFETY:
            // - `payload.len()` is an exact multiple of `size_of::<As>()`.
            // - `len` is `payload.len() / size_of::<As>()`.
            // - `index < len`, so `start < payload.len()`.
            let remaining = unsafe { self.payload.get_unchecked(start..) };
            let item = As::get(remaining);
            self.index += 1;
            Some(item)
        }
    }
}

unsafe impl<'de, C: ConfigCore> SchemaReadContext<'de, C, PrimitiveTy> for PrimitiveValue {
    type Dst = Self;

    #[inline]
    fn read_with_context(
        ctx: PrimitiveTy,
        reader: impl Reader<'de>,
        dst: &mut MaybeUninit<Self::Dst>,
    ) -> ReadResult<()> {
        let val = match ctx {
            PrimitiveTy::U8 => <u8 as SchemaRead<C>>::get(reader).map(PrimitiveValue::U8)?,
            PrimitiveTy::U16 => <u16 as SchemaRead<C>>::get(reader).map(PrimitiveValue::U16)?,
            PrimitiveTy::U32 => <u32 as SchemaRead<C>>::get(reader).map(PrimitiveValue::U32)?,
            PrimitiveTy::U64 => <u64 as SchemaRead<C>>::get(reader).map(PrimitiveValue::U64)?,
            PrimitiveTy::I8 => <i8 as SchemaRead<C>>::get(reader).map(PrimitiveValue::I8)?,
            PrimitiveTy::I16 => <i16 as SchemaRead<C>>::get(reader).map(PrimitiveValue::I16)?,
            PrimitiveTy::I32 => <i32 as SchemaRead<C>>::get(reader).map(PrimitiveValue::I32)?,
            PrimitiveTy::I64 => <i64 as SchemaRead<C>>::get(reader).map(PrimitiveValue::I64)?,
            PrimitiveTy::F32 => <f32 as SchemaRead<C>>::get(reader).map(PrimitiveValue::F32)?,
            PrimitiveTy::F64 => <f64 as SchemaRead<C>>::get(reader).map(PrimitiveValue::F64)?,
            PrimitiveTy::Bool => <bool as SchemaRead<C>>::get(reader).map(PrimitiveValue::Bool)?,
        };

        dst.write(val);

        Ok(())
    }
}

unsafe impl<'de> SchemaReadContext<'de, DefaultConfig, PrimitiveTy> for usize {
    type Dst = Self;

    #[inline]
    fn read_with_context(
        ctx: PrimitiveTy,
        reader: impl Reader<'de>,
        dst: &mut MaybeUninit<Self::Dst>,
    ) -> ReadResult<()> {
        #[inline]
        fn try_cast_to_usize(val: impl TryInto<usize>) -> ReadResult<usize> {
            #[cold]
            fn err() -> ReadError {
                ReadError::Custom("cannot cast to usize")
            }
            val.try_into().map_err(|_| err())
        }

        let val = match ctx {
            PrimitiveTy::U8 => <u8 as SchemaRead<DefaultConfig>>::get(reader).map(usize::from)?,
            PrimitiveTy::U16 => <u16 as SchemaRead<DefaultConfig>>::get(reader).map(usize::from)?,
            PrimitiveTy::U32 => {
                <u32 as SchemaRead<DefaultConfig>>::get(reader).and_then(try_cast_to_usize)?
            }
            PrimitiveTy::U64 => {
                <u64 as SchemaRead<DefaultConfig>>::get(reader).and_then(try_cast_to_usize)?
            }
            PrimitiveTy::I8 => {
                <i8 as SchemaRead<DefaultConfig>>::get(reader).and_then(try_cast_to_usize)?
            }
            PrimitiveTy::I16 => {
                <i16 as SchemaRead<DefaultConfig>>::get(reader).and_then(try_cast_to_usize)?
            }
            PrimitiveTy::I32 => {
                <i32 as SchemaRead<DefaultConfig>>::get(reader).and_then(try_cast_to_usize)?
            }
            PrimitiveTy::I64 => {
                <i64 as SchemaRead<DefaultConfig>>::get(reader).and_then(try_cast_to_usize)?
            }
            _ => return Err(ReadError::Custom("cannot cast to usize")),
        };

        dst.write(val);

        Ok(())
    }
}
unsafe impl<'de, C: ConfigCore> SchemaReadContext<'de, C, PrimitiveTy> for Value<'de> {
    type Dst = Self;

    #[inline]
    fn read_with_context(
        ctx: PrimitiveTy,
        reader: impl Reader<'de>,
        dst: &mut MaybeUninit<Self::Dst>,
    ) -> ReadResult<()> {
        let val = match ctx {
            PrimitiveTy::U8 => <u8 as SchemaRead<C>>::get(reader).map(Value::U8)?,
            PrimitiveTy::U16 => <u16 as SchemaRead<C>>::get(reader).map(Value::U16)?,
            PrimitiveTy::U32 => <u32 as SchemaRead<C>>::get(reader).map(Value::U32)?,
            PrimitiveTy::U64 => <u64 as SchemaRead<C>>::get(reader).map(Value::U64)?,
            PrimitiveTy::I8 => <i8 as SchemaRead<C>>::get(reader).map(Value::I8)?,
            PrimitiveTy::I16 => <i16 as SchemaRead<C>>::get(reader).map(Value::I16)?,
            PrimitiveTy::I32 => <i32 as SchemaRead<C>>::get(reader).map(Value::I32)?,
            PrimitiveTy::I64 => <i64 as SchemaRead<C>>::get(reader).map(Value::I64)?,
            PrimitiveTy::F32 => <f32 as SchemaRead<C>>::get(reader).map(Value::F32)?,
            PrimitiveTy::F64 => <f64 as SchemaRead<C>>::get(reader).map(Value::F64)?,
            PrimitiveTy::Bool => <bool as SchemaRead<C>>::get(reader).map(Value::Bool)?,
        };

        dst.write(val);

        Ok(())
    }
}

unsafe impl<'de> SchemaReadContext<'de, DefaultConfig, PrimitiveTy> for Vec<PrimitiveValue> {
    type Dst = Self;

    #[inline]
    fn read_with_context(
        ctx: PrimitiveTy,
        reader: impl Reader<'de>,
        dst: &mut MaybeUninit<Self::Dst>,
    ) -> ReadResult<()> {
        match ctx {
            PrimitiveTy::U8 => {
                <Vec<PrimitiveValue> as SchemaReadContext<DefaultConfig, _>>::read_with_context(
                    Map::new(PrimitiveValue::U8),
                    reader,
                    dst,
                )
            }
            PrimitiveTy::U16 => {
                <Vec<PrimitiveValue> as SchemaReadContext<DefaultConfig, _>>::read_with_context(
                    Map::new(PrimitiveValue::U16),
                    reader,
                    dst,
                )
            }
            PrimitiveTy::U32 => {
                <Vec<PrimitiveValue> as SchemaReadContext<DefaultConfig, _>>::read_with_context(
                    Map::new(PrimitiveValue::U32),
                    reader,
                    dst,
                )
            }
            PrimitiveTy::U64 => {
                <Vec<PrimitiveValue> as SchemaReadContext<DefaultConfig, _>>::read_with_context(
                    Map::new(PrimitiveValue::U64),
                    reader,
                    dst,
                )
            }
            PrimitiveTy::I8 => {
                <Vec<PrimitiveValue> as SchemaReadContext<DefaultConfig, _>>::read_with_context(
                    Map::new(PrimitiveValue::I8),
                    reader,
                    dst,
                )
            }
            PrimitiveTy::I16 => {
                <Vec<PrimitiveValue> as SchemaReadContext<DefaultConfig, _>>::read_with_context(
                    Map::new(PrimitiveValue::I16),
                    reader,
                    dst,
                )
            }
            PrimitiveTy::I32 => {
                <Vec<PrimitiveValue> as SchemaReadContext<DefaultConfig, _>>::read_with_context(
                    Map::new(PrimitiveValue::I32),
                    reader,
                    dst,
                )
            }
            PrimitiveTy::I64 => {
                <Vec<PrimitiveValue> as SchemaReadContext<DefaultConfig, _>>::read_with_context(
                    Map::new(PrimitiveValue::I64),
                    reader,
                    dst,
                )
            }
            PrimitiveTy::F32 => {
                <Vec<PrimitiveValue> as SchemaReadContext<DefaultConfig, _>>::read_with_context(
                    Map::new(PrimitiveValue::F32),
                    reader,
                    dst,
                )
            }
            PrimitiveTy::F64 => {
                <Vec<PrimitiveValue> as SchemaReadContext<DefaultConfig, _>>::read_with_context(
                    Map::new(PrimitiveValue::F64),
                    reader,
                    dst,
                )
            }
            PrimitiveTy::Bool => {
                <Vec<PrimitiveValue> as SchemaReadContext<DefaultConfig, _>>::read_with_context(
                    Map::new(PrimitiveValue::Bool),
                    reader,
                    dst,
                )
            }
        }
    }
}

pub(crate) struct Array<T> {
    _marker: core::marker::PhantomData<T>,
}

unsafe impl<'de, C: ConfigCore> SchemaReadContext<'de, C, (Len, PrimitiveTy)>
    for Array<PrimitiveValue>
{
    type Dst = Vec<PrimitiveValue>;

    #[inline]
    fn read_with_context(
        ctx: (Len, PrimitiveTy),
        reader: impl Reader<'de>,
        dst: &mut MaybeUninit<Self::Dst>,
    ) -> ReadResult<()> {
        let (Len(len), ty) = ctx;

        match ty {
            PrimitiveTy::U8 => {
                <Vec<PrimitiveValue> as SchemaReadContext<DefaultConfig, _>>::read_with_context(
                    LenMap::new(len, PrimitiveValue::U8),
                    reader,
                    dst,
                )
            }
            PrimitiveTy::U16 => {
                <Vec<PrimitiveValue> as SchemaReadContext<DefaultConfig, _>>::read_with_context(
                    LenMap::new(len, PrimitiveValue::U16),
                    reader,
                    dst,
                )
            }
            PrimitiveTy::U32 => {
                <Vec<PrimitiveValue> as SchemaReadContext<DefaultConfig, _>>::read_with_context(
                    LenMap::new(len, PrimitiveValue::U32),
                    reader,
                    dst,
                )
            }
            PrimitiveTy::U64 => {
                <Vec<PrimitiveValue> as SchemaReadContext<DefaultConfig, _>>::read_with_context(
                    LenMap::new(len, PrimitiveValue::U64),
                    reader,
                    dst,
                )
            }
            PrimitiveTy::I8 => {
                <Vec<PrimitiveValue> as SchemaReadContext<DefaultConfig, _>>::read_with_context(
                    LenMap::new(len, PrimitiveValue::I8),
                    reader,
                    dst,
                )
            }
            PrimitiveTy::I16 => {
                <Vec<PrimitiveValue> as SchemaReadContext<DefaultConfig, _>>::read_with_context(
                    LenMap::new(len, PrimitiveValue::I16),
                    reader,
                    dst,
                )
            }
            PrimitiveTy::I32 => {
                <Vec<PrimitiveValue> as SchemaReadContext<DefaultConfig, _>>::read_with_context(
                    LenMap::new(len, PrimitiveValue::I32),
                    reader,
                    dst,
                )
            }
            PrimitiveTy::I64 => {
                <Vec<PrimitiveValue> as SchemaReadContext<DefaultConfig, _>>::read_with_context(
                    LenMap::new(len, PrimitiveValue::I64),
                    reader,
                    dst,
                )
            }
            PrimitiveTy::F32 => {
                <Vec<PrimitiveValue> as SchemaReadContext<DefaultConfig, _>>::read_with_context(
                    LenMap::new(len, PrimitiveValue::F32),
                    reader,
                    dst,
                )
            }
            PrimitiveTy::F64 => {
                <Vec<PrimitiveValue> as SchemaReadContext<DefaultConfig, _>>::read_with_context(
                    LenMap::new(len, PrimitiveValue::F64),
                    reader,
                    dst,
                )
            }
            PrimitiveTy::Bool => {
                <Vec<PrimitiveValue> as SchemaReadContext<DefaultConfig, _>>::read_with_context(
                    LenMap::new(len, PrimitiveValue::Bool),
                    reader,
                    dst,
                )
            }
        }
    }
}
