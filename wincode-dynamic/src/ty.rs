use {
    crate::{Array, PrimitiveValue, Value},
    std::borrow::Cow,
    wincode::{
        ReadResult, SchemaRead, SchemaReadContext, SchemaWrite, config::DefaultConfig,
        context::Len, io::Reader,
    },
};

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
    pub fn parse<'de>(self, reader: impl Reader<'de>) -> ReadResult<Value<'de>> {
        match self {
            Ty::PrimitiveTy(ty) => {
                <Value as SchemaReadContext<DefaultConfig, _>>::get_with_context(ty, reader)
            }
            Ty::String => {
                <Cow<str> as SchemaRead<'de, DefaultConfig>>::get(reader).map(Value::String)
            }
            Ty::Vec {
                ty: PrimitiveTy::U8,
            } => <Cow<[u8]> as SchemaRead<'de, DefaultConfig>>::get(reader).map(Value::Bytes),

            Ty::Vec { ty } => {
                <Vec<PrimitiveValue> as SchemaReadContext<'de, DefaultConfig, _>>::get_with_context(
                    ty, reader,
                )
                .map(Value::Vec)
            }
            Ty::Array {
                ty: PrimitiveTy::U8,
                len,
            } => <Cow<[u8]> as SchemaReadContext<'de, DefaultConfig, Len>>::get_with_context(
                Len(len),
                reader,
            )
            .map(Value::Bytes),
            Ty::Array { ty, len } => <Array<PrimitiveValue> as SchemaReadContext<
                'de,
                DefaultConfig,
                _,
            >>::get_with_context((Len(len), ty), reader)
            .map(Value::Vec),
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
