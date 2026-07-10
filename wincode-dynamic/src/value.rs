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
            PrimitiveTy::U8 => {
                <u16 as SchemaRead<'de, DefaultConfig>>::get(reader).map(PrimitiveValue::U16)?
            }
            PrimitiveTy::U16 => {
                <u16 as SchemaRead<'de, DefaultConfig>>::get(reader).map(PrimitiveValue::U16)?
            }
            PrimitiveTy::U32 => {
                <u32 as SchemaRead<'de, DefaultConfig>>::get(reader).map(PrimitiveValue::U32)?
            }
            PrimitiveTy::U64 => {
                <u64 as SchemaRead<'de, DefaultConfig>>::get(reader).map(PrimitiveValue::U64)?
            }
            PrimitiveTy::I8 => {
                <i8 as SchemaRead<'de, DefaultConfig>>::get(reader).map(PrimitiveValue::I8)?
            }
            PrimitiveTy::I16 => {
                <i16 as SchemaRead<'de, DefaultConfig>>::get(reader).map(PrimitiveValue::I16)?
            }
            PrimitiveTy::I32 => {
                <i32 as SchemaRead<'de, DefaultConfig>>::get(reader).map(PrimitiveValue::I32)?
            }
            PrimitiveTy::I64 => {
                <i64 as SchemaRead<'de, DefaultConfig>>::get(reader).map(PrimitiveValue::I64)?
            }
            PrimitiveTy::F32 => {
                <f32 as SchemaRead<'de, DefaultConfig>>::get(reader).map(PrimitiveValue::F32)?
            }
            PrimitiveTy::F64 => {
                <f64 as SchemaRead<'de, DefaultConfig>>::get(reader).map(PrimitiveValue::F64)?
            }
            PrimitiveTy::Bool => {
                <bool as SchemaRead<'de, DefaultConfig>>::get(reader).map(PrimitiveValue::Bool)?
            }
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
            PrimitiveTy::U8 => {
                <u16 as SchemaRead<'de, DefaultConfig>>::get(reader).map(Value::U16)?
            }
            PrimitiveTy::U16 => {
                <u16 as SchemaRead<'de, DefaultConfig>>::get(reader).map(Value::U16)?
            }
            PrimitiveTy::U32 => {
                <u32 as SchemaRead<'de, DefaultConfig>>::get(reader).map(Value::U32)?
            }
            PrimitiveTy::U64 => {
                <u64 as SchemaRead<'de, DefaultConfig>>::get(reader).map(Value::U64)?
            }
            PrimitiveTy::I8 => {
                <i8 as SchemaRead<'de, DefaultConfig>>::get(reader).map(Value::I8)?
            }
            PrimitiveTy::I16 => {
                <i16 as SchemaRead<'de, DefaultConfig>>::get(reader).map(Value::I16)?
            }
            PrimitiveTy::I32 => {
                <i32 as SchemaRead<'de, DefaultConfig>>::get(reader).map(Value::I32)?
            }
            PrimitiveTy::I64 => {
                <i64 as SchemaRead<'de, DefaultConfig>>::get(reader).map(Value::I64)?
            }
            PrimitiveTy::F32 => {
                <f32 as SchemaRead<'de, DefaultConfig>>::get(reader).map(Value::F32)?
            }
            PrimitiveTy::F64 => {
                <f64 as SchemaRead<'de, DefaultConfig>>::get(reader).map(Value::F64)?
            }
            PrimitiveTy::Bool => {
                <bool as SchemaRead<'de, DefaultConfig>>::get(reader).map(Value::Bool)?
            }
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
        let val = match ctx {
            PrimitiveTy::U8 => {
                <Vec<PrimitiveValue> as SchemaReadContext<'de, DefaultConfig, _>>::get_with_context(
                    Map::<u8, _, _>::new(PrimitiveValue::U8),
                    reader,
                )
            }
            PrimitiveTy::U16 => {
                <Vec<PrimitiveValue> as SchemaReadContext<'de, DefaultConfig, _>>::get_with_context(
                    Map::<u16, _, _>::new(PrimitiveValue::U16),
                    reader,
                )
            }
            PrimitiveTy::U32 => {
                <Vec<PrimitiveValue> as SchemaReadContext<'de, DefaultConfig, _>>::get_with_context(
                    Map::<u32, _, _>::new(PrimitiveValue::U32),
                    reader,
                )
            }
            PrimitiveTy::U64 => {
                <Vec<PrimitiveValue> as SchemaReadContext<'de, DefaultConfig, _>>::get_with_context(
                    Map::<u64, _, _>::new(PrimitiveValue::U64),
                    reader,
                )
            }
            PrimitiveTy::I8 => {
                <Vec<PrimitiveValue> as SchemaReadContext<'de, DefaultConfig, _>>::get_with_context(
                    Map::<i8, _, _>::new(PrimitiveValue::I8),
                    reader,
                )
            }
            PrimitiveTy::I16 => {
                <Vec<PrimitiveValue> as SchemaReadContext<'de, DefaultConfig, _>>::get_with_context(
                    Map::<i16, _, _>::new(PrimitiveValue::I16),
                    reader,
                )
            }
            PrimitiveTy::I32 => {
                <Vec<PrimitiveValue> as SchemaReadContext<'de, DefaultConfig, _>>::get_with_context(
                    Map::<i32, _, _>::new(PrimitiveValue::I32),
                    reader,
                )
            }
            PrimitiveTy::I64 => {
                <Vec<PrimitiveValue> as SchemaReadContext<'de, DefaultConfig, _>>::get_with_context(
                    Map::<i64, _, _>::new(PrimitiveValue::I64),
                    reader,
                )
            }
            PrimitiveTy::F32 => {
                <Vec<PrimitiveValue> as SchemaReadContext<'de, DefaultConfig, _>>::get_with_context(
                    Map::<f32, _, _>::new(PrimitiveValue::F32),
                    reader,
                )
            }
            PrimitiveTy::F64 => {
                <Vec<PrimitiveValue> as SchemaReadContext<'de, DefaultConfig, _>>::get_with_context(
                    Map::<f64, _, _>::new(PrimitiveValue::F64),
                    reader,
                )
            }
            PrimitiveTy::Bool => {
                <Vec<PrimitiveValue> as SchemaReadContext<'de, DefaultConfig, _>>::get_with_context(
                    Map::<bool, _, _>::new(PrimitiveValue::Bool),
                    reader,
                )
            }
        }?;

        dst.write(val);

        Ok(())
    }
}

pub(crate) struct Array<T> {
    _marker: core::marker::PhantomData<T>,
}

unsafe impl<'de> SchemaReadContext<'de, DefaultConfig, (Len, PrimitiveTy)>
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
        let val = match ty {
            PrimitiveTy::U8 => {
                <Vec<PrimitiveValue> as SchemaReadContext<'de, DefaultConfig, _>>::get_with_context(
                    LenMap::<u8, _, _>::new(len, PrimitiveValue::U8),
                    reader,
                )
            }
            PrimitiveTy::U16 => {
                <Vec<PrimitiveValue> as SchemaReadContext<'de, DefaultConfig, _>>::get_with_context(
                    LenMap::<u16, _, _>::new(len, PrimitiveValue::U16),
                    reader,
                )
            }
            PrimitiveTy::U32 => {
                <Vec<PrimitiveValue> as SchemaReadContext<'de, DefaultConfig, _>>::get_with_context(
                    LenMap::<u32, _, _>::new(len, PrimitiveValue::U32),
                    reader,
                )
            }
            PrimitiveTy::U64 => {
                <Vec<PrimitiveValue> as SchemaReadContext<'de, DefaultConfig, _>>::get_with_context(
                    LenMap::<u64, _, _>::new(len, PrimitiveValue::U64),
                    reader,
                )
            }
            PrimitiveTy::I8 => {
                <Vec<PrimitiveValue> as SchemaReadContext<'de, DefaultConfig, _>>::get_with_context(
                    LenMap::<i8, _, _>::new(len, PrimitiveValue::I8),
                    reader,
                )
            }
            PrimitiveTy::I16 => {
                <Vec<PrimitiveValue> as SchemaReadContext<'de, DefaultConfig, _>>::get_with_context(
                    LenMap::<i16, _, _>::new(len, PrimitiveValue::I16),
                    reader,
                )
            }
            PrimitiveTy::I32 => {
                <Vec<PrimitiveValue> as SchemaReadContext<'de, DefaultConfig, _>>::get_with_context(
                    LenMap::<i32, _, _>::new(len, PrimitiveValue::I32),
                    reader,
                )
            }
            PrimitiveTy::I64 => {
                <Vec<PrimitiveValue> as SchemaReadContext<'de, DefaultConfig, _>>::get_with_context(
                    LenMap::<i64, _, _>::new(len, PrimitiveValue::I64),
                    reader,
                )
            }
            PrimitiveTy::F32 => {
                <Vec<PrimitiveValue> as SchemaReadContext<'de, DefaultConfig, _>>::get_with_context(
                    LenMap::<f32, _, _>::new(len, PrimitiveValue::F32),
                    reader,
                )
            }
            PrimitiveTy::F64 => {
                <Vec<PrimitiveValue> as SchemaReadContext<'de, DefaultConfig, _>>::get_with_context(
                    LenMap::<f64, _, _>::new(len, PrimitiveValue::F64),
                    reader,
                )
            }
            PrimitiveTy::Bool => {
                <Vec<PrimitiveValue> as SchemaReadContext<'de, DefaultConfig, _>>::get_with_context(
                    LenMap::<bool, _, _>::new(len, PrimitiveValue::Bool),
                    reader,
                )
            }
        }?;

        dst.write(val);

        Ok(())
    }
}
