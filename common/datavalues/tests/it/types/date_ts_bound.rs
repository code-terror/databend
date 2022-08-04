// Copyright 2022 Datafuse Labs.
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

use common_datavalues::prelude::*;
use common_exception::Result;
use pretty_assertions::assert_eq;

#[test]
fn test_bump_datetime() -> Result<()> {
    use std::ops::Sub;

    use chrono::offset::TimeZone;
    use chrono::NaiveDate;
    use chrono_tz::Tz;
    // timestamp microseconds
    {
        let tz: Tz = "UTC".parse().unwrap();
        let dt = tz.ymd(9999, 12, 31).and_hms_micro(23, 59, 59, 999999);
        assert_eq!(dt.timestamp_micros(), TIMESTAMP_MAX);
        let dt = tz.ymd(1000, 1, 1).and_hms_micro(0, 0, 0, 0);
        assert_eq!(dt.timestamp_micros(), TIMESTAMP_MIN);
    }
    // date
    {
        let epoch = NaiveDate::from_ymd(1970, 1, 1);
        let tz: Tz = "UTC".parse().unwrap();
        let dt = tz.ymd(9999, 12, 31);
        let duration = dt.naive_utc().sub(epoch);
        assert_eq!(duration.num_days(), DATE_MAX as i64);

        let epoch = NaiveDate::from_ymd(1970, 1, 1);
        let tz: Tz = "UTC".parse().unwrap();
        let dt = tz.ymd(1000, 1, 1);
        let duration = dt.naive_utc().sub(epoch);
        assert_eq!(duration.num_days(), DATE_MIN as i64);
    }

    {
        // 1022-05-16 03:25:02.868894
        let tz: Tz = "UTC".parse().unwrap();
        let dt = tz.ymd(1022, 5, 16).and_hms_micro(3, 25, 2, 868894);
        let dt2 = DateConverter::to_timestamp(&dt.timestamp_micros(), &tz);
        assert_eq!(dt, dt2);
    }
    Ok(())
}
