//! wincode extensions

use {
    alloc::vec::Vec,
    core::{marker::PhantomData, mem::MaybeUninit},
    wincode::{
        ReadResult, SchemaRead, SchemaReadContext, TypeMeta,
        config::{Config, DefaultConfig},
        io::Reader,
        len::SeqLen,
    },
};

// Map adapter that reads a sequence of `A` values and maps them to `B` values
// using `f`.
//
// Avoids intermediate collects and writes directly as `Vec<B>`.
//
// TODO: upstream
pub(crate) struct Map<A, F> {
    f: F,
    _marker: PhantomData<A>,
}

impl<A, F> Map<A, F> {
    pub(crate) fn new<B>(f: F) -> Self
    where
        F: FnMut(A) -> B,
    {
        Self {
            f,
            _marker: PhantomData,
        }
    }
}

unsafe impl<'de, A, B, F> SchemaReadContext<'de, DefaultConfig, Map<A, F>> for Vec<B>
where
    A: SchemaRead<'de, DefaultConfig>,
    F: FnMut(A::Dst) -> B,
{
    type Dst = Vec<B>;

    #[inline]
    fn read_with_context(
        ctx: Map<A, F>,
        mut reader: impl Reader<'de>,
        dst: &mut MaybeUninit<Self::Dst>,
    ) -> ReadResult<()> {
        let len = <<DefaultConfig as Config>::LengthEncoding as SeqLen<DefaultConfig>>::read_prealloc_check::<B>(
            reader.by_ref(),
        )?;
        <Vec<B> as SchemaReadContext<'de, DefaultConfig, LenMap<A, F>>>::read_with_context(
            LenMap {
                len,
                f: ctx.f,
                _marker: PhantomData,
            },
            reader,
            dst,
        )
    }
}

pub(crate) struct LenMap<A, F> {
    len: usize,
    f: F,
    _marker: PhantomData<A>,
}

impl<A, F> LenMap<A, F> {
    pub fn new<B>(len: usize, f: F) -> Self
    where
        F: FnMut(A) -> B,
    {
        Self {
            len,
            f,
            _marker: PhantomData,
        }
    }
}

unsafe impl<'de, A, B, F> SchemaReadContext<'de, DefaultConfig, LenMap<A, F>> for Vec<B>
where
    A: SchemaRead<'de, DefaultConfig>,
    F: FnMut(A::Dst) -> B,
{
    type Dst = Vec<B>;

    #[inline]
    fn read_with_context(
        ctx: LenMap<A, F>,
        mut reader: impl Reader<'de>,
        dst: &mut MaybeUninit<Self::Dst>,
    ) -> ReadResult<()> {
        let LenMap { len, mut f, .. } = ctx;

        let mut vec = Vec::with_capacity(len);
        let ptr: *mut B = vec.as_mut_ptr();

        // TODO: make drop safe
        match A::TYPE_META {
            TypeMeta::Static { size, .. } => {
                let mut reader = unsafe { reader.as_trusted_for_seq(len, size) }?;
                for i in 0..len {
                    let val = A::get(reader.by_ref())?;
                    let mapped = (f)(val);
                    unsafe { ptr.add(i).write(mapped) };
                    unsafe { vec.set_len(i + 1) }
                }
            }
            TypeMeta::Dynamic => {
                for i in 0..len {
                    let val = A::get(reader.by_ref())?;
                    let mapped = (f)(val);
                    unsafe { ptr.add(i).write(mapped) };
                    unsafe { vec.set_len(i + 1) }
                }
            }
        }

        dst.write(vec);
        Ok(())
    }
}
