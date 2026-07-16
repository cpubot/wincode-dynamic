//! wincode extensions
//!

use {
    alloc::vec::Vec,
    core::{marker::PhantomData, mem::MaybeUninit},
    wincode::{
        ReadError, ReadResult, SchemaRead, SchemaReadContext, TypeMeta,
        config::{Config, ConfigCore, DefaultConfig},
        context::Len,
        error::read_length_encoding_overflow,
        io::Reader,
        len::SeqLen,
    },
};

// Map adapter that reads a sequence of `A` values and maps them to `B` values using `f`.
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

pub mod lazy_slice {
    use {super::*, std::ops::Index};

    /// A borrowed view over an encoded sequence of fixed-width values.
    ///
    /// Unlike `&[T]`, this type does not form references to its elements. Each
    /// element is decoded by value when it is requested, so the encoded bytes
    /// do not need to satisfy `T`'s alignment. The view itself does not allocate,
    /// although a custom element decoder may do so.
    ///
    /// Deserialization requires a reader that supports borrows from its backing
    /// storage and rejects element types whose [`TypeMeta`] is dynamic.
    #[derive(Debug, Clone, PartialEq, Eq, Copy)]
    pub struct LazySlice<'a, T, C = DefaultConfig> {
        bytes: &'a [u8],
        len: usize,
        element_size: usize,
        _marker: PhantomData<&'a [T]>,
        _config: PhantomData<C>,
    }

    impl<'a, T, C> LazySlice<'a, T, C> {
        /// Returns the number of encoded elements.
        #[inline]
        pub const fn len(&self) -> usize {
            self.len
        }

        /// Returns whether the encoded sequence contains no elements.
        #[inline]
        pub const fn is_empty(&self) -> bool {
            self.len == 0
        }

        /// Returns the encoded element bytes, excluding the length prefix.
        #[inline]
        pub const fn as_bytes(&self) -> &'a [u8] {
            self.bytes
        }
    }

    impl<'a, T, C> LazySlice<'a, T, C>
    where
        C: ConfigCore,
        T: SchemaRead<'a, C>,
    {
        /// Returns an iterator that decodes elements by value.
        #[inline]
        pub const fn iter(&self) -> LazySliceIter<'a, T, C> {
            LazySliceIter {
                slice: self.bytes,
                offset: 0,
                remaining: self.len,
                element_size: self.element_size,
                _marker: PhantomData,
                _config: PhantomData,
            }
        }
    }

    unsafe impl<'de, T, C: Config> SchemaRead<'de, C> for LazySlice<'de, T, C>
    where
        T: SchemaRead<'de, C>,
    {
        type Dst = Self;

        #[inline]
        fn read(mut reader: impl Reader<'de>, dst: &mut MaybeUninit<Self::Dst>) -> ReadResult<()> {
            #[cold]
            fn unsupported_dynamic() -> ReadError {
                ReadError::Custom("LazySlice does not support dynamic types")
            }

            let size = match T::TYPE_META {
                TypeMeta::Static { size, .. } => size,
                TypeMeta::Dynamic => return Err(unsupported_dynamic()),
            };
            let len = C::LengthEncoding::read(reader.by_ref())?;
            let Some(num_bytes) = len.checked_mul(size) else {
                return Err(read_length_encoding_overflow("usize::MAX"));
            };

            let bytes = <&'de [u8] as SchemaReadContext<'de, C, _>>::get_with_context(
                Len(num_bytes),
                reader,
            )?;

            dst.write(Self {
                bytes,
                len,
                element_size: size,
                _marker: PhantomData,
                _config: PhantomData,
            });

            Ok(())
        }
    }

    /// An iterator over the decoded elements of a [`LazySlice`].
    pub struct LazySliceIter<'a, T, C> {
        slice: &'a [u8],
        offset: usize,
        remaining: usize,
        element_size: usize,
        _marker: PhantomData<&'a [T]>,
        _config: PhantomData<C>,
    }

    impl<'a, T, C> IntoIterator for LazySlice<'a, T, C>
    where
        C: ConfigCore,
        T: SchemaRead<'a, C>,
    {
        type Item = ReadResult<T::Dst>;
        type IntoIter = LazySliceIter<'a, T, C>;

        #[inline]
        fn into_iter(self) -> Self::IntoIter {
            self.iter()
        }
    }

    impl<'a, T, C> IntoIterator for &LazySlice<'a, T, C>
    where
        C: ConfigCore,
        T: SchemaRead<'a, C>,
    {
        type Item = ReadResult<T::Dst>;
        type IntoIter = LazySliceIter<'a, T, C>;

        #[inline]
        fn into_iter(self) -> Self::IntoIter {
            self.iter()
        }
    }

    impl<'a, T, C: ConfigCore> Iterator for LazySliceIter<'a, T, C>
    where
        T: SchemaRead<'a, C>,
    {
        type Item = ReadResult<T::Dst>;

        #[inline]
        fn next(&mut self) -> Option<Self::Item> {
            if self.remaining == 0 {
                return None;
            }

            // SAFETY: `LazySlice::read` borrows exactly `len * element_size`
            // bytes, and `remaining` limits this iterator to `len` chunks.
            let buf = unsafe { self.slice.get_unchecked(self.offset..) };

            // Advance before decoding so a malformed element is returned once
            // rather than stalling the iterator on the same error forever.
            self.offset += self.element_size;
            self.remaining -= 1;
            Some(T::get(buf))
        }

        #[inline]
        fn size_hint(&self) -> (usize, Option<usize>) {
            (self.remaining, Some(self.remaining))
        }
    }

    impl<'a, T, C: ConfigCore> ExactSizeIterator for LazySliceIter<'a, T, C>
    where
        T: SchemaRead<'a, C>,
    {
        #[inline]
        fn len(&self) -> usize {
            self.remaining
        }
    }

    impl<'a, T, C: ConfigCore> core::iter::FusedIterator for LazySliceIter<'a, T, C> where
        T: SchemaRead<'a, C>
    {
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[derive(SchemaRead)]
        struct Test<'a> {
            value: LazySlice<'a, u64>,
        }

        #[test]
        fn decodes_fixed_width_elements() {
            let data = vec![3u64; 16];
            let buf = wincode::serialize(&data).unwrap();
            let test: Test<'_> = wincode::deserialize(&buf).unwrap();

            assert_eq!(test.value.len(), data.len());
            assert!(!test.value.is_empty());
            assert_eq!(
                test.value.iter().collect::<ReadResult<Vec<_>>>().unwrap(),
                data
            );
        }

        #[test]
        fn decodes_unaligned_elements() {
            let data = vec![1u64, 2, 3];
            let encoded = wincode::serialize(&data).unwrap();
            let element_offset = encoded.len() - data.len() * core::mem::size_of::<u64>();
            let alignment = core::mem::align_of::<u64>();
            let mut storage = vec![0; encoded.len() + alignment];
            let storage_address = storage.as_ptr() as usize;
            let start = (0..alignment)
                .find(|start| !(storage_address + start + element_offset).is_multiple_of(alignment))
                .unwrap();
            storage[start..start + encoded.len()].copy_from_slice(&encoded);
            let input = &storage[start..start + encoded.len()];

            assert_ne!(input[element_offset..].as_ptr() as usize % alignment, 0);
            let values: LazySlice<'_, u64> = wincode::deserialize(input).unwrap();
            assert_eq!(values.iter().collect::<ReadResult<Vec<_>>>().unwrap(), data);
        }

        #[test]
        fn preserves_zero_sized_element_count() {
            let data = vec![(); 4];
            let encoded = wincode::serialize(&data).unwrap();
            let values: LazySlice<'_, ()> = wincode::deserialize(&encoded).unwrap();

            assert_eq!(values.len(), data.len());
            assert!(values.as_bytes().is_empty());
            assert_eq!(values.iter().collect::<ReadResult<Vec<_>>>().unwrap(), data);
        }

        #[test]
        fn handles_an_empty_sequence() {
            let encoded = wincode::serialize(&Vec::<u64>::new()).unwrap();
            let values: LazySlice<'_, u64> = wincode::deserialize(&encoded).unwrap();
            let mut iter = values.iter();

            assert_eq!(values.len(), 0);
            assert!(values.is_empty());
            assert!(values.as_bytes().is_empty());
            assert_eq!(iter.len(), 0);
            assert!(iter.next().is_none());
        }

        #[test]
        fn rejects_truncated_element_bytes() {
            let mut encoded = wincode::serialize(&vec![1u64, 2]).unwrap();
            encoded.pop();
            let result: ReadResult<LazySlice<'_, u64>> = wincode::deserialize(&encoded);

            assert!(matches!(
                result,
                Err(ReadError::Io(wincode::io::ReadError::ReadSizeLimit(_)))
            ));
        }

        #[test]
        fn advances_after_an_invalid_element() {
            let mut encoded = wincode::serialize(&vec![false, true]).unwrap();
            let element_offset = encoded.len() - 2;
            encoded[element_offset] = 2;
            let values: LazySlice<'_, bool> = wincode::deserialize(&encoded).unwrap();
            let mut iter = values.iter();

            assert_eq!(iter.len(), 2);
            assert!(matches!(
                iter.next().unwrap(),
                Err(ReadError::InvalidBoolEncoding(2))
            ));
            assert_eq!(iter.len(), 1);
            assert!(iter.next().unwrap().unwrap());
            assert!(iter.next().is_none());
            assert!(iter.next().is_none());
        }
    }
}
