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
//
// Borrow from apache/arrow/rust/datafusion/src/sql/sql_parser
// See notice.md

use sqlparser::keywords::Keyword;
use sqlparser::parser::ParserError;

use crate::sql::statements::DfDeleteStatement;
use crate::sql::DfParser;
use crate::sql::DfStatement;

impl<'a> DfParser<'a> {
    // DELETE.
    pub(crate) fn parse_delete(&mut self) -> Result<DfStatement<'a>, ParserError> {
        self.parser.next_token();
        let parser = &mut self.parser;
        parser.expect_keyword(Keyword::FROM)?;
        let table_name = parser.parse_object_name()?;
        let selection = if parser.parse_keyword(Keyword::WHERE) {
            Some(parser.parse_expr()?)
        } else {
            None
        };

        Ok(DfStatement::Delete(Box::new(DfDeleteStatement {
            name: table_name,
            selection,
        })))
    }
}
