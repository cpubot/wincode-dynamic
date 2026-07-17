#![allow(dead_code)]

/// Deliberately invalid metadata must fail instead of wrapping the serialized size.
///
/// ```compile_fail,E0080
/// use {
///     core::mem::MaybeUninit,
///     wincode::{
///         ReadResult, SchemaRead, SchemaWrite, TypeMeta, WriteResult,
///         config::ConfigCore,
///         io::{Reader, Writer},
///     },
///     wincode_dynamic::{DynTy, PrimitiveTy, SchemaDynamic, SerializedSize, Ty},
/// };
///
/// struct Huge;
///
/// impl DynTy for Huge {
///     const TYPE: Ty = Ty::PrimitiveTy(PrimitiveTy::U8);
/// }
///
/// unsafe impl<C: ConfigCore> SchemaWrite<C> for Huge {
///     type Src = Self;
///
///     const TYPE_META: TypeMeta = TypeMeta::Static {
///         size: usize::MAX,
///         zero_copy: false,
///     };
///
///     fn size_of(_: &Self::Src) -> WriteResult<usize> {
///         Ok(usize::MAX)
///     }
///
///     fn write(_: impl Writer, _: &Self::Src) -> WriteResult<()> {
///         unreachable!()
///     }
/// }
///
/// unsafe impl<'de, C: ConfigCore> SchemaRead<'de, C> for Huge {
///     type Dst = Self;
///
///     const TYPE_META: TypeMeta = TypeMeta::Static {
///         size: usize::MAX,
///         zero_copy: false,
///     };
///
///     fn read(_: impl Reader<'de>, _: &mut MaybeUninit<Self::Dst>) -> ReadResult<()> {
///         unreachable!()
///     }
/// }
///
/// #[derive(SchemaDynamic)]
/// enum Overflow {
///     Huge(Huge),
///     Byte(u8),
/// }
///
/// unsafe impl<C: ConfigCore> SchemaWrite<C> for Overflow {
///     type Src = Self;
///
///     const TYPE_META: TypeMeta = TypeMeta::Dynamic;
///
///     fn size_of(_: &Self::Src) -> WriteResult<usize> {
///         unreachable!()
///     }
///
///     fn write(_: impl Writer, _: &Self::Src) -> WriteResult<()> {
///         unreachable!()
///     }
/// }
///
/// unsafe impl<'de, C: ConfigCore> SchemaRead<'de, C> for Overflow {
///     type Dst = Self;
///
///     const TYPE_META: TypeMeta = TypeMeta::Dynamic;
///
///     fn read(_: impl Reader<'de>, _: &mut MaybeUninit<Self::Dst>) -> ReadResult<()> {
///         unreachable!()
///     }
/// }
///
/// const _: SerializedSize = Overflow::SERIALIZED_SIZE;
/// ```
fn serialized_size_overflow() {}
