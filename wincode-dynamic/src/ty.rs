use {
    crate::{LazyVec, Value},
    alloc::{borrow::Cow, string::String, vec::Vec},
    wincode::{
        ReadResult, SchemaRead, SchemaReadContext, SchemaWrite,
        config::{Config, DefaultConfig},
        context::Len,
        error::read_length_encoding_overflow,
        io::{BorrowKind, Reader},
        len::SeqLen,
    },
};

type DefaultLengthEncoding = <DefaultConfig as Config>::LengthEncoding;

#[inline]
fn read_byte_payload<'de>(byte_len: usize, reader: impl Reader<'de>) -> ReadResult<Cow<'de, [u8]>> {
    if !reader.supports_borrow(BorrowKind::Backing) {
        <DefaultLengthEncoding as SeqLen<DefaultConfig>>::prealloc_check::<u8>(byte_len)?;
    }

    <Cow<'de, [u8]> as SchemaReadContext<'de, DefaultConfig, Len>>::get_with_context(
        Len(byte_len),
        reader,
    )
}

#[inline]
fn read_primitive_payload<'de>(
    ty: PrimitiveTy,
    len: usize,
    reader: impl Reader<'de>,
) -> ReadResult<Value<'de>> {
    let byte_len = len
        .checked_mul(ty.size())
        .ok_or_else(|| read_length_encoding_overflow("usize::MAX"))?;

    read_byte_payload(byte_len, reader).map(|payload| {
        // SAFETY: `byte_len` was checked as `len * ty.size()`, and
        // `read_byte_payload` returned exactly `byte_len` bytes.
        Value::Vec(unsafe { LazyVec::new_unchecked(ty, len, payload) })
    })
}

/// A fixed-width primitive type supported by dynamic decoding.
///
/// Each variant identifies both the Rust value kind and its wincode wire
/// representation under [`DefaultConfig`].
#[derive(Clone, Copy, Debug, PartialEq, SchemaRead, SchemaWrite)]
#[wincode(tag_encoding = "u8")]
pub enum PrimitiveTy {
    /// An unsigned 8-bit integer.
    U8,
    /// An unsigned 16-bit integer.
    U16,
    /// An unsigned 32-bit integer.
    U32,
    /// An unsigned 64-bit integer.
    U64,
    /// A signed 8-bit integer.
    I8,
    /// A signed 16-bit integer.
    I16,
    /// A signed 32-bit integer.
    I32,
    /// A signed 64-bit integer.
    I64,
    /// A 32-bit floating-point number.
    F32,
    /// A 64-bit floating-point number.
    F64,
    /// A Boolean value.
    Bool,
}

impl PrimitiveTy {
    #[inline]
    pub(crate) fn parse_into_usize<'de>(self, reader: impl Reader<'de>) -> ReadResult<usize> {
        <usize as SchemaReadContext<DefaultConfig, _>>::get_with_context(self, reader)
    }

    /// Parses a [`Value`] of this type from the given [`Reader`].
    ///
    /// Decoding uses wincode's [`DefaultConfig`].
    ///
    /// # Examples
    ///
    /// ```
    /// use wincode_dynamic::{PrimitiveTy, Value};
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let encoded = wincode::serialize(&42u64)?;
    /// let value = PrimitiveTy::U64.parse(&encoded[..])?;
    ///
    /// assert_eq!(value, Value::U64(42));
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn parse<'de>(self, reader: impl Reader<'de>) -> ReadResult<Value<'de>> {
        <Value as SchemaReadContext<DefaultConfig, _>>::get_with_context(self, reader)
    }

    /// Returns the complete encoded width of this primitive, in bytes.
    ///
    /// Primitive encodings have no length prefix, so this is also the complete
    /// serialized size under wincode's [`DefaultConfig`].
    #[inline]
    pub const fn size(self) -> usize {
        match self {
            PrimitiveTy::U8 => 1,
            PrimitiveTy::U16 => 2,
            PrimitiveTy::U32 => 4,
            PrimitiveTy::U64 => 8,
            PrimitiveTy::I8 => 1,
            PrimitiveTy::I16 => 2,
            PrimitiveTy::I32 => 4,
            PrimitiveTy::I64 => 8,
            PrimitiveTy::F32 => 4,
            PrimitiveTy::F64 => 8,
            PrimitiveTy::Bool => 1,
        }
    }
}

/// A field type supported by runtime schemas and dynamic decoding.
///
/// `Ty` describes the wire representation of a field under wincode's
/// [`DefaultConfig`]. It is generated from a Rust field type through [`DynTy`].
#[derive(Clone, Copy, Debug, PartialEq, SchemaRead, SchemaWrite)]
#[wincode(tag_encoding = "u8")]
pub enum Ty {
    /// A scalar primitive.
    PrimitiveTy(PrimitiveTy),
    /// A length-prefixed UTF-8 string.
    String,
    /// A length-prefixed sequence of primitive elements.
    Vec(PrimitiveTy),
    /// A fixed-length array of primitive elements.
    Array {
        /// The element type.
        ty: PrimitiveTy,
        /// The number of elements.
        len: usize,
    },
}

impl Ty {
    /// Parses a [`Value`] of this type from the given [`Reader`].
    ///
    /// Decoding uses wincode's [`DefaultConfig`]. Strings and byte sequences
    /// remain borrowed when the reader supports stable borrowing; other
    /// primitive sequences are returned as a [`LazyVec`].
    ///
    /// # Examples
    ///
    /// ```
    /// use wincode_dynamic::{PrimitiveTy, Ty, Value};
    /// # use std::borrow::Cow;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let encoded = wincode::serialize("w1nc0d3")?;
    /// let value = Ty::String.parse(&encoded[..])?;
    ///
    /// assert_eq!(value, Value::String(Cow::Borrowed("w1nc0d3")));
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn parse<'de>(self, mut reader: impl Reader<'de>) -> ReadResult<Value<'de>> {
        match self {
            Ty::PrimitiveTy(ty) => {
                <Value as SchemaReadContext<DefaultConfig, _>>::get_with_context(ty, reader)
            }
            Ty::String => {
                <Cow<'de, str> as SchemaRead<'de, DefaultConfig>>::get(reader).map(Value::String)
            }
            Ty::Vec(PrimitiveTy::U8) => {
                <Cow<[u8]> as SchemaRead<'de, DefaultConfig>>::get(reader).map(Value::Bytes)
            }
            Ty::Vec(ty) => {
                let len = <DefaultLengthEncoding as SeqLen<DefaultConfig>>::read(reader.by_ref())?;
                read_primitive_payload(ty, len, reader)
            }
            Ty::Array {
                ty: PrimitiveTy::U8,
                len,
            } => read_byte_payload(len, reader).map(Value::Bytes),
            Ty::Array { ty, len } => read_primitive_payload(ty, len, reader),
        }
    }

    /// Returns the complete serialized size of this type when it is fixed.
    ///
    /// Primitive types and arrays have a fixed size under wincode's
    /// [`DefaultConfig`]. Strings and vectors return `None` because their
    /// complete size, including the length prefix, depends on the value.
    #[inline]
    pub const fn size(self) -> Option<usize> {
        match self {
            Ty::PrimitiveTy(ty) => Some(ty.size()),
            Ty::String => None,
            Ty::Vec(..) => None,
            Ty::Array { ty, len } => Some(ty.size() * len),
        }
    }
}

/// Associates a supported Rust primitive with its runtime [`PrimitiveTy`].
pub trait DynPrimitiveTy {
    /// The runtime primitive type corresponding to `Self`.
    const TYPE: PrimitiveTy;
}

impl<T> DynTy for T
where
    T: DynPrimitiveTy,
{
    const TYPE: Ty = Ty::PrimitiveTy(T::TYPE);
}

/// Associates a Rust field type with its runtime [`Ty`].
pub trait DynTy {
    /// The runtime type corresponding to `Self`.
    const TYPE: Ty;
}

impl DynPrimitiveTy for u8 {
    const TYPE: PrimitiveTy = PrimitiveTy::U8;
}

impl DynPrimitiveTy for u16 {
    const TYPE: PrimitiveTy = PrimitiveTy::U16;
}

impl DynPrimitiveTy for u32 {
    const TYPE: PrimitiveTy = PrimitiveTy::U32;
}

impl DynPrimitiveTy for u64 {
    const TYPE: PrimitiveTy = PrimitiveTy::U64;
}

impl DynPrimitiveTy for i8 {
    const TYPE: PrimitiveTy = PrimitiveTy::I8;
}

impl DynPrimitiveTy for i16 {
    const TYPE: PrimitiveTy = PrimitiveTy::I16;
}

impl DynPrimitiveTy for i32 {
    const TYPE: PrimitiveTy = PrimitiveTy::I32;
}

impl DynPrimitiveTy for i64 {
    const TYPE: PrimitiveTy = PrimitiveTy::I64;
}

impl DynPrimitiveTy for f32 {
    const TYPE: PrimitiveTy = PrimitiveTy::F32;
}

impl DynPrimitiveTy for f64 {
    const TYPE: PrimitiveTy = PrimitiveTy::F64;
}

impl DynPrimitiveTy for bool {
    const TYPE: PrimitiveTy = PrimitiveTy::Bool;
}

impl DynTy for String {
    const TYPE: Ty = Ty::String;
}

impl<T> DynTy for Vec<T>
where
    T: DynPrimitiveTy,
{
    const TYPE: Ty = Ty::Vec(T::TYPE);
}

impl<const N: usize, T: DynPrimitiveTy> DynTy for [T; N] {
    const TYPE: Ty = Ty::Array {
        ty: T::TYPE,
        len: N,
    };
}
