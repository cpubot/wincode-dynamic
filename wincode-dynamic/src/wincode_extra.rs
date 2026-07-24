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

#[cfg(test)]
mod tests {
    use {
        super::*,
        core::cell::Cell,
        std::panic::{AssertUnwindSafe, catch_unwind},
    };

    struct DynamicU8;

    unsafe impl<'de> SchemaRead<'de, DefaultConfig> for DynamicU8 {
        type Dst = u8;

        fn read(reader: impl Reader<'de>, dst: &mut MaybeUninit<Self::Dst>) -> ReadResult<()> {
            <u8 as SchemaRead<DefaultConfig>>::read(reader, dst)
        }
    }

    struct DropSpy<'a>(&'a Cell<usize>);

    impl Drop for DropSpy<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[test]
    fn len_map_drops_initialized_values_on_read_error() {
        let drops = Cell::new(0);
        let mut dst = MaybeUninit::<Vec<DropSpy<'_>>>::uninit();
        let ctx = LenMap::<DynamicU8, _> {
            len: 3,
            f: |_| DropSpy(&drops),
            _marker: PhantomData,
        };

        let result = <Vec<DropSpy<'_>> as SchemaReadContext<DefaultConfig, _>>::read_with_context(
            ctx,
            &[1, 2][..],
            &mut dst,
        );

        assert!(result.is_err());
        assert_eq!(drops.get(), 2);
    }

    #[test]
    fn len_map_drops_initialized_values_when_mapper_panics() {
        let drops = Cell::new(0);
        let calls = Cell::new(0);
        let mut dst = MaybeUninit::<Vec<DropSpy<'_>>>::uninit();
        let ctx = LenMap::<DynamicU8, _> {
            len: 3,
            f: |_| {
                calls.set(calls.get() + 1);
                assert_ne!(calls.get(), 3, "mapper panic");
                DropSpy(&drops)
            },
            _marker: PhantomData,
        };

        let result = catch_unwind(AssertUnwindSafe(|| {
            <Vec<DropSpy<'_>> as SchemaReadContext<DefaultConfig, _>>::read_with_context(
                ctx,
                &[1, 2, 3][..],
                &mut dst,
            )
        }));

        assert!(result.is_err());
        assert_eq!(drops.get(), 2);
    }

    #[test]
    fn static_len_map_drops_initialized_values_on_read_error() {
        let drops = Cell::new(0);
        let mut dst = MaybeUninit::<Vec<DropSpy<'_>>>::uninit();
        let ctx = LenMap::<bool, _>::new(2, |_| DropSpy(&drops));

        let result = <Vec<DropSpy<'_>> as SchemaReadContext<DefaultConfig, _>>::read_with_context(
            ctx,
            &[1, 2][..],
            &mut dst,
        );

        assert!(matches!(
            result,
            Err(wincode::ReadError::InvalidBoolEncoding(2))
        ));
        assert_eq!(drops.get(), 1);
    }

    #[test]
    fn static_len_map_drops_each_initialized_value_when_mapper_panics() {
        let drops = [Cell::new(0), Cell::new(0), Cell::new(0)];
        let mut dst = MaybeUninit::<Vec<DropSpy<'_>>>::uninit();
        let ctx = LenMap::<u8, _>::new(3, |value| {
            assert_ne!(value, 3, "mapper panic");
            DropSpy(&drops[usize::from(value - 1)])
        });

        let result = catch_unwind(AssertUnwindSafe(|| {
            <Vec<DropSpy<'_>> as SchemaReadContext<DefaultConfig, _>>::read_with_context(
                ctx,
                &[1, 2, 3][..],
                &mut dst,
            )
        }));

        assert!(result.is_err());
        assert_eq!(drops.map(|drop| drop.get()), [1, 1, 0]);
    }
}
