use {
    crate::{
        PrimitiveTy,
        wincode_extra::{LenMap, Map},
    },
    alloc::{borrow::Cow, vec::Vec},
    core::{marker::PhantomData, mem::MaybeUninit},
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

impl PrimitiveValue {
    #[inline]
    pub const fn size(self) -> usize {
        match self {
            PrimitiveValue::U8(_) => 1,
            PrimitiveValue::U16(_) => 2,
            PrimitiveValue::U32(_) => 4,
            PrimitiveValue::U64(_) => 8,
            PrimitiveValue::I8(_) => 1,
            PrimitiveValue::I16(_) => 2,
            PrimitiveValue::I32(_) => 4,
            PrimitiveValue::I64(_) => 8,
            PrimitiveValue::F32(_) => 4,
            PrimitiveValue::F64(_) => 8,
            PrimitiveValue::Bool(_) => 1,
        }
    }
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

impl Value<'_> {
    #[inline]
    pub fn size(&self) -> usize {
        match self {
            Value::U8(_) => 1,
            Value::U16(_) => 2,
            Value::U32(_) => 4,
            Value::U64(_) => 8,
            Value::I8(_) => 1,
            Value::I16(_) => 2,
            Value::I32(_) => 4,
            Value::I64(_) => 8,
            Value::F32(_) => 4,
            Value::F64(_) => 8,
            Value::Bool(_) => 1,
            Value::String(str) => str.len(),
            Value::Bytes(bytes) => bytes.len(),
            Value::Vec(vec) => vec.payload.len(),
        }
    }
}

/// A lazily decoded vector of fixed-width primitive values.
///
/// The encoded payload remains borrowed when the input reader supports stable
/// borrowing. Use [`Self::try_iter_as`] to iterate by reference when the
/// concrete element type is known, [`Self::try_into_iter_as`] to create an
/// owning iterator, or [`Self::into_dyn_vec`] to decode dynamically typed
/// values.
#[derive(Debug, Clone, PartialEq)]
pub struct LazyVec<'a> {
    ty: PrimitiveTy,
    payload: Cow<'a, [u8]>,
    /// The length of the vector, in _elements_ (not bytes).
    ///
    /// Invariant: `payload.len() == len * ty.size()`.
    len: usize,
}

/// Lazy-vector iterator types.
pub mod lazy_vec {
    use {super::*, crate::DynPrimitiveTy};

    /// A borrowing, lazy iterator over the elements of a [`LazyVec`].
    ///
    /// Each item is decoded only when [`Iterator::next`] is called.
    #[derive(Debug, Clone, Copy)]
    pub struct Iter<'a, As> {
        payload: &'a [u8],
        len: usize,
        index: usize,
        _marker: PhantomData<As>,
    }

    /// An owning, lazy iterator over the elements of a [`LazyVec`].
    ///
    /// Each item is decoded only when [`Iterator::next`] is called.
    #[derive(Debug, Clone)]
    pub struct IntoIter<'a, As> {
        payload: Cow<'a, [u8]>,
        len: usize,
        index: usize,
        _marker: PhantomData<As>,
    }

    impl<'a> LazyVec<'a> {
        /// # Safety
        ///
        /// `payload.len()` must equal `len * ty.size()`.
        pub(crate) unsafe fn new_unchecked(
            ty: PrimitiveTy,
            len: usize,
            payload: Cow<'a, [u8]>,
        ) -> Self {
            Self { ty, len, payload }
        }

        /// Returns a borrowing, lazy iterator of `As` values.
        ///
        /// The requested type must match the primitive element type recorded in
        /// the schema. This prevents, for example, interpreting the payload of a
        /// `Vec<u64>` as a sequence of `u8` values.
        ///
        /// This method validates the element type and its width, but it does
        /// not decode any elements. Decoding happens as the returned iterator is
        /// advanced, and each item is returned as a [`ReadResult`]. Consequently,
        /// malformed element encodings are reported by the iterator item that
        /// encounters them rather than by this method.
        ///
        /// The iterator borrows this lazy vector. Multiple iterators can be
        /// created without cloning or decoding the payload.
        ///
        /// To iterate, `As` must also implement
        /// [`SchemaRead<DefaultConfig, Dst = As>`](SchemaRead).
        ///
        /// # Errors
        ///
        /// Returns an error if:
        ///
        /// - `As` does not match the vector's recorded primitive element type; or
        /// - the size of `As` does not match the recorded element width.
        ///
        /// # Examples
        ///
        /// ```
        /// use wincode::{ReadResult, SchemaRead, SchemaWrite};
        /// use wincode_dynamic::{Decoder, SchemaDynamic, Value};
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
        /// let decoder = Decoder::new(Message::schema());
        /// let mut fields = decoder.fields(encoded.as_slice())?;
        ///
        /// let field = fields.next().expect("values field")?;
        /// assert_eq!(field.name(), "values");
        /// let Value::Vec(values) = field.value() else {
        ///     panic!("expected a vector");
        /// };
        /// let values = values
        ///     .try_iter_as::<u64>()?
        ///     .collect::<ReadResult<Vec<_>>>()?;
        ///
        /// assert_eq!(values, [10, 20, 30]);
        /// # Ok::<(), wincode::ReadError>(())
        /// ```
        #[inline]
        pub fn try_iter_as<'iter, As>(&'iter self) -> ReadResult<Iter<'iter, As>>
        where
            As: DynPrimitiveTy,
        {
            validate_element_type::<As>(self.ty)?;

            Ok(Iter {
                len: self.len,
                payload: self.payload.as_ref(),
                index: 0,
                _marker: PhantomData,
            })
        }

        /// Converts this vector into an owning, lazy iterator of `As` values.
        ///
        /// This performs the same type validation as [`Self::try_iter_as`], but
        /// consumes the lazy vector and moves its payload into the iterator.
        /// Borrowed payloads remain borrowed, and owned payloads are moved without
        /// cloning or reallocating them.
        ///
        /// # Errors
        ///
        /// Returns an error if `As` does not match the vector's recorded
        /// primitive element type or width.
        #[inline]
        pub fn try_into_iter_as<As>(self) -> ReadResult<IntoIter<'a, As>>
        where
            As: DynPrimitiveTy,
        {
            validate_element_type::<As>(self.ty)?;

            Ok(IntoIter {
                len: self.len,
                payload: self.payload,
                index: 0,
                _marker: PhantomData,
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
            <<DefaultConfig as Config>::LengthEncoding as SeqLen<DefaultConfig>>::prealloc_check::<
                PrimitiveValue,
            >(self.len)?;
            <Array<PrimitiveValue> as SchemaReadContext<DefaultConfig, _>>::get_with_context(
                (Len(self.len), self.ty),
                &self.payload[..],
            )
        }

        /// Returns the number of elements in the vector.
        #[inline]
        pub const fn len(&self) -> usize {
            self.len
        }

        /// Returns `true` if the vector contains no elements.
        #[inline]
        pub fn is_empty(&self) -> bool {
            self.payload.is_empty()
        }

        #[cfg(all(test, feature = "std"))]
        pub(crate) fn has_borrowed_payload(&self) -> bool {
            matches!(self.payload, Cow::Borrowed(_))
        }

        /// Returns the vector's primitive element type.
        #[inline]
        pub const fn ty(&self) -> PrimitiveTy {
            self.ty
        }
    }

    #[inline]
    fn validate_element_type<As: DynPrimitiveTy>(ty: PrimitiveTy) -> ReadResult<()> {
        #[cold]
        const fn ty_mismatch() -> ReadError {
            ReadError::Custom("lazy vector element type mismatch")
        }

        if As::TYPE != ty || ty.size() != size_of::<As>() {
            return Err(ty_mismatch());
        }

        Ok(())
    }

    macro_rules! impl_iterator {
        ($iter:ident) => {
            impl<As> Iterator for $iter<'_, As>
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
                    let remaining = unsafe { self.payload.as_ref().get_unchecked(start..) };
                    let item = As::get(remaining);
                    self.index += 1;
                    Some(item)
                }

                #[inline]
                fn size_hint(&self) -> (usize, Option<usize>) {
                    let remaining = self.len - self.index;
                    (remaining, Some(remaining))
                }
            }

            impl<As> ExactSizeIterator for $iter<'_, As>
            where
                As: for<'de> SchemaRead<'de, DefaultConfig, Dst = As>,
            {
                #[inline]
                fn len(&self) -> usize {
                    self.len - self.index
                }
            }

            impl<As> core::iter::FusedIterator for $iter<'_, As> where
                As: for<'de> SchemaRead<'de, DefaultConfig, Dst = As>
            {
            }
        };
    }

    impl_iterator!(Iter);
    impl_iterator!(IntoIter);
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
