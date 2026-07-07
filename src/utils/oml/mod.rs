//! OML 数据处理与格式化模块。
//!
//! 对外保留两类能力：
//! - 基于 OML 模型的 `DataRecord` 异步转换；
//! - OML 文本格式化。

mod format;
mod transform;

pub use format::{OmlFormatError, OmlFormatter};
pub use transform::convert_record;
