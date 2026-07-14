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

#[derive(Clone, Copy, Debug, PartialEq, SchemaRead, SchemaWrite)]
#[wincode(tag_encoding = "u8")]
pub enum PrimitiveTy {
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
    Bool,
}

impl PrimitiveTy {
    #[inline]
    pub(crate) fn parse_into_usize<'de>(self, reader: impl Reader<'de>) -> ReadResult<usize> {
        <usize as SchemaReadContext<DefaultConfig, _>>::get_with_context(self, reader)
    }

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

#[derive(Clone, Copy, Debug, PartialEq, SchemaRead, SchemaWrite)]
#[wincode(tag_encoding = "u8")]
pub enum Ty {
    PrimitiveTy(PrimitiveTy),
    String,
    Vec { ty: PrimitiveTy },
    Array { ty: PrimitiveTy, len: usize },
}

impl Ty {
    #[inline]
    pub fn parse<'de>(self, mut reader: impl Reader<'de>) -> ReadResult<Value<'de>> {
        match self {
            Ty::PrimitiveTy(ty) => {
                <Value as SchemaReadContext<DefaultConfig, _>>::get_with_context(ty, reader)
            }
            Ty::String => {
                <Cow<'de, str> as SchemaRead<'de, DefaultConfig>>::get(reader).map(Value::String)
            }
            Ty::Vec {
                ty: PrimitiveTy::U8,
            } => <Cow<[u8]> as SchemaRead<'de, DefaultConfig>>::get(reader).map(Value::Bytes),
            Ty::Vec { ty } => {
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
}

pub trait DynPrimitiveTy {
    const TYPE: PrimitiveTy;
}

impl<T> DynTy for T
where
    T: DynPrimitiveTy,
{
    const TYPE: Ty = Ty::PrimitiveTy(T::TYPE);
}

pub trait DynTy {
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
    const TYPE: Ty = Ty::Vec { ty: T::TYPE };
}

impl<const N: usize, T: DynPrimitiveTy> DynTy for [T; N] {
    const TYPE: Ty = Ty::Array {
        ty: T::TYPE,
        len: N,
    };
}
