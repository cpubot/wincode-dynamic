//! wincode extensions
//!

use {
    std::mem::MaybeUninit,
    wincode::{
        ReadResult, SchemaRead, SchemaReadContext, TypeMeta,
        config::{Config, DefaultConfig},
        io::Reader,
        len::SeqLen,
    },
};

// Map adapter that reads a sequence of `A` values and maps them to `B` values using `f`.
//
// Avoids intermediate collects and writes directly as `Vec<B>`.
//
// TODO: upstream
pub(crate) struct Map<A, B, F> {
    f: F,
    _marker: std::marker::PhantomData<(A, B)>,
}

impl<A, B, F> Map<A, B, F> {
    pub(crate) fn new(f: F) -> Self {
        Self {
            f,
            _marker: std::marker::PhantomData,
        }
    }
}

unsafe impl<'de, A, B, F> SchemaReadContext<'de, DefaultConfig, Map<A, B, F>> for Vec<B>
where
    F: Fn(A::Dst) -> B,
    A: SchemaRead<'de, DefaultConfig>,
{
    type Dst = Vec<B>;

    fn read_with_context(
        ctx: Map<A, B, F>,
        mut reader: impl Reader<'de>,
        dst: &mut MaybeUninit<Self::Dst>,
    ) -> ReadResult<()> {
        let len = <<DefaultConfig as Config>::LengthEncoding as SeqLen<DefaultConfig>>::read_prealloc_check::<B>(
            reader.by_ref(),
        )?;
        let mut vec = Vec::with_capacity(len);
        let ptr: *mut B = vec.as_mut_ptr();

        // TODO: make drop safe
        match A::TYPE_META {
            TypeMeta::Static { size, .. } => {
                let mut reader = unsafe { reader.as_trusted_for_seq(len, size) }?;
                for i in 0..len {
                    let val = A::get(reader.by_ref())?;
                    let mapped = (ctx.f)(val);
                    unsafe { ptr.add(i).write(mapped) };
                }
            }
            TypeMeta::Dynamic => {
                for i in 0..len {
                    let val = A::get(reader.by_ref())?;
                    let mapped = (ctx.f)(val);
                    unsafe { ptr.add(i).write(mapped) };
                }
            }
        }

        unsafe { vec.set_len(len) }

        dst.write(vec);
        Ok(())
    }
}
