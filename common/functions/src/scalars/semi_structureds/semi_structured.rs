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

use common_datavalues::TypeID;

use super::as_type::as_function_creator;
use super::get::GetFunction;
use super::get::GetIgnoreCaseFunction;
use super::get_path::GetPathFunction;
use super::json_extract_path_text::JsonExtractPathTextFunction;
use super::parse_json::ParseJsonFunction;
use super::parse_json::TryParseJsonFunction;
use super::ObjectKeysFunction;
use crate::scalars::CheckJsonFunction;
use crate::scalars::FunctionFactory;

pub struct SemiStructuredFunction;

impl SemiStructuredFunction {
    pub fn register(factory: &mut FunctionFactory) {
        factory.register("as_boolean", as_function_creator(TypeID::Boolean));
        factory.register("as_integer", as_function_creator(TypeID::Int64));
        factory.register("as_float", as_function_creator(TypeID::Float64));
        factory.register("as_string", as_function_creator(TypeID::String));
        factory.register("as_array", as_function_creator(TypeID::VariantArray));
        factory.register("as_object", as_function_creator(TypeID::VariantObject));
        factory.register("parse_json", ParseJsonFunction::desc());
        factory.register("try_parse_json", TryParseJsonFunction::desc());
        factory.register("check_json", CheckJsonFunction::desc());
        factory.register("get", GetFunction::desc());
        factory.register("get_ignore_case", GetIgnoreCaseFunction::desc());
        factory.register("get_path", GetPathFunction::desc());
        factory.register(
            "json_extract_path_text",
            JsonExtractPathTextFunction::desc(),
        );
        factory.register("object_keys", ObjectKeysFunction::desc())
    }
}
