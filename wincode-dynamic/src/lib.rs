use {
    crate::wincode_extra::Map,
    core::mem::MaybeUninit,
    wincode::{
        ReadResult, SchemaRead, SchemaReadContext, SchemaWrite,
        config::{ConfigCore, DefaultConfig},
        io::Reader,
    },
};

mod wincode_extra;
pub use wincode_dynamic_derive::*;

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
    pub fn parse<'de>(self, reader: impl Reader<'de>) -> ReadResult<Value> {
        match self {
            Ty::PrimitiveTy(ty) => {
                <Value as SchemaReadContext<DefaultConfig, _>>::get_with_context(ty, reader)
            }
            Ty::String => {
                <String as SchemaRead<'de, DefaultConfig>>::get(reader).map(Value::String)
            }
            Ty::Vec { ty } => {
                <Vec<PrimitiveValue> as SchemaReadContext<'de, DefaultConfig, _>>::get_with_context(
                    ty, reader,
                )
                .map(Value::Vec)
            }
            #[expect(unused)]
            Ty::Array { ty, len } => {
                todo!()
            }
        }
    }
}

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

unsafe impl<'de, C: ConfigCore> SchemaReadContext<'de, C, PrimitiveTy> for PrimitiveValue {
    type Dst = Self;

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

unsafe impl<'de, C: ConfigCore> SchemaReadContext<'de, C, PrimitiveTy> for Value {
    type Dst = Self;

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

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
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
    String(String),
    Vec(Vec<PrimitiveValue>),
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

#[derive(SchemaRead, SchemaWrite, Debug, Clone)]
pub struct Field {
    name: String,
    ty: Ty,
}

impl Field {
    pub fn new(name: impl Into<String>, ty: Ty) -> Self {
        Self {
            name: name.into(),
            ty,
        }
    }

    #[inline]
    pub fn parse<'de>(&self, reader: impl Reader<'de>) -> ReadResult<Value> {
        self.ty.parse(reader)
    }
}

#[derive(SchemaRead, SchemaWrite, Debug, Clone)]
pub struct Header {
    name: String,
    fields: Box<[Field]>,
}

impl Header {
    pub fn new(name: impl Into<String>, fields: Box<[Field]>) -> Self {
        Self {
            name: name.into(),
            fields,
        }
    }
}

pub trait SchemaDynamic {
    fn schema() -> Header;
}

pub struct SchemaRuntime {
    header: Header,
}

impl SchemaRuntime {
    pub fn new(header: Header) -> Self {
        Self { header }
    }

    pub fn name(&self) -> &str {
        &self.header.name
    }

    pub fn parse<'de>(&self, mut reader: impl Reader<'de>) -> ReadResult<Vec<Value>> {
        self.header
            .fields
            .iter()
            .map(|field| field.parse(reader.by_ref()))
            .collect()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn example() {
        #[derive(SchemaDynamic, SchemaRead, SchemaWrite, PartialEq, Debug)]
        #[wincode_dynamic(internal)]
        struct Hello {
            a: u64,
            b: bool,
            vals: Vec<u64>,
            str: String,
        }

        let hello = Hello {
            a: 42,
            b: true,
            vals: vec![333; 4],
            str: String::from("hello world"),
        };

        let schema = Hello::schema();
        let dyn_parser = SchemaRuntime::new(schema);

        let payload = wincode::serialize(&hello).unwrap();
        let result = dyn_parser.parse(&payload[..]).unwrap();
        assert_eq!(
            result,
            vec![
                Value::U64(42),
                Value::Bool(true),
                Value::Vec(vec![PrimitiveValue::U64(333); 4]),
                Value::String(String::from("hello world"))
            ]
        )
    }
}
