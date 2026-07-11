use {
    crate::{
        PrimitiveTy,
        wincode_extra::{LenMap, Map},
    },
    core::mem::MaybeUninit,
    std::borrow::Cow,
    wincode::{
        ReadResult, SchemaRead, SchemaReadContext,
        config::{ConfigCore, DefaultConfig},
        context::Len,
        io::Reader,
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
    Vec(Vec<PrimitiveValue>),
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
