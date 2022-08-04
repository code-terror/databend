// Copyright 2021 Datafuse Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
use std::fmt::Debug;

use common_arrow::arrow::bitmap::Bitmap;
use common_exception::ErrorCode;
use common_exception::Result;
use common_io::prelude::*;
use micromarshal::Marshal;

use crate::prelude::*;

impl Series {
    //  Group by (nullable(u16), nullable(u8)) needs 16 + 8 + 8 + 8 = 40 bytes, then we pad the bytes  up to u64 to store the hash value.
    //  If the value is null, we write 1 to the null_offset, otherwise we write 0.
    //  since most value is not null, so this can make the hash number as low as possible.
    //
    //  u16 column pos       │u8 column pos
    // │                     │
    // │                     │
    // │                     │                       ┌─  null offset of u8 column
    // ▼                    ▼                      ▼
    // ┌──────────┬──────────┬───────────┬───────────┬───────────┬───────────┬─────────┬─────────┐
    // │   1byte  │   1byte  │    1byte  │    1byte  │   1byte   │    1byte  │   1byte │   1byte │
    // └──────────┴──────────┴───────────┴───────────┴───────────┼───────────┴─────────┴─────────┤
    //                                   ▲                       │                               │
    //                                   │                       └──────────►       ◄────────────┘
    //                                   │                                    unused bytes
    //                                   └─  null offset of u16 column
    pub fn fixed_hash(
        column: &ColumnRef,
        ptr: *mut u8,
        step: usize,
        // (null_offset, bitmap)
        nulls: Option<(usize, Option<Bitmap>)>,
    ) -> Result<()> {
        let column = column.convert_full_column();

        if column.data_type().is_nullable() {
            let (null_offset, bitmap) = nulls.unwrap();
            let (_, validity) = column.validity();
            let bitmap = combine_validities_2(bitmap, validity.cloned());
            let column = Series::remove_nullable(&column);

            return Series::fixed_hash(&column, ptr, step, Some((null_offset, bitmap)));
        }

        let column = Series::remove_nullable(&column);
        let type_id = column.data_type_id().to_physical_type();

        with_match_physical_primitive_type!(type_id, |$T| {
            let col: &<$T as Scalar>::ColumnType = Series::check_get(&column)?;
            GroupHash::fixed_hash(col, ptr, step, nulls)
        }, {
            if type_id == PhysicalTypeID::Boolean {
                let col: &BooleanColumn = Series::check_get(&column)?;
                GroupHash::fixed_hash(col, ptr, step, nulls)
            } else {
                Err(ErrorCode::BadDataValueType(
                    format!("Unsupported apply fn fixed_hash operation for column: {:?}", column.data_type()),
                ))
            }
        })
    }
}

// Read more:
//  https://www.cockroachlabs.com/blog/vectorized-hash-joiner/
//  http://myeyesareblind.com/2017/02/06/Combine-hash-values/

pub trait GroupHash: Debug {
    /// Compute the hash for all values in the array.
    fn fixed_hash(
        &self,
        _ptr: *mut u8,
        _step: usize,
        _nulls: Option<(usize, Option<Bitmap>)>,
    ) -> Result<()> {
        Err(ErrorCode::BadDataValueType(format!(
            "Unsupported apply fn fixed_hash operation for {:?}",
            self,
        )))
    }
}

impl<T> GroupHash for PrimitiveColumn<T>
where
    T: PrimitiveType,
    T: Marshal + StatBuffer + Sized,
{
    fn fixed_hash(
        &self,
        ptr: *mut u8,
        step: usize,
        nulls: Option<(usize, Option<Bitmap>)>,
    ) -> Result<()> {
        let mut ptr = ptr;

        match nulls {
            Some((offsize, Some(bitmap))) => {
                for (value, valid) in self.iter().zip(bitmap.iter()) {
                    unsafe {
                        if valid {
                            let slice =
                                std::slice::from_raw_parts_mut(ptr, std::mem::size_of::<T>());
                            value.marshal(slice);
                        } else {
                            ptr.add(offsize).write(1u8);
                        }

                        ptr = ptr.add(step);
                    }
                }
            }
            _ => {
                for value in self.iter() {
                    unsafe {
                        let slice = std::slice::from_raw_parts_mut(ptr, std::mem::size_of::<T>());
                        value.marshal(slice);
                        ptr = ptr.add(step);
                    }
                }
            }
        }

        Ok(())
    }
}

impl GroupHash for BooleanColumn {
    fn fixed_hash(
        &self,
        ptr: *mut u8,
        step: usize,
        nulls: Option<(usize, Option<Bitmap>)>,
    ) -> Result<()> {
        let mut ptr = ptr;

        match nulls {
            Some((offsize, Some(bitmap))) => {
                for (value, valid) in self.iter().zip(bitmap.iter()) {
                    unsafe {
                        if valid {
                            std::ptr::copy_nonoverlapping(&(value as u8) as *const u8, ptr, 1);
                        } else {
                            ptr.add(offsize).write(1u8);
                        }
                        ptr = ptr.add(step);
                    }
                }
            }
            _ => {
                for value in self.iter() {
                    unsafe {
                        std::ptr::copy_nonoverlapping(&(value as u8) as *const u8, ptr, 1);
                        ptr = ptr.add(step);
                    }
                }
            }
        }

        Ok(())
    }
}
