//! 音频缓存相关错误

/// 缓存操作错误类型
#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum CacheError {
    /// 缓存目录不可用
    #[error("缓存目录不可用")]
    DirUnavailable,

    /// 提交临时文件失败
    #[error("提交临时文件失败: {0}")]
    CommitTmp(String),

    /// 索引加载失败
    #[allow(dead_code)]
    #[error("加载缓存索引失败: {source}")]
    LoadIndex {
        #[source]
        source: std::io::Error,
    },

    /// 索引保存失败
    #[allow(dead_code)]
    #[error("保存缓存索引失败: {source}")]
    SaveIndex {
        #[source]
        source: std::io::Error,
    },

    /// 缓存大小超限
    #[allow(dead_code)]
    #[error("缓存大小超限: current={current}MB, limit={limit}MB")]
    SizeLimit { current: u64, limit: u64 },

    /// 文件操作失败
    #[error("文件操作失败: {0}")]
    FileOp(#[from] std::io::Error),

    /// 序列化失败
    #[allow(dead_code)]
    #[error("序列化失败: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_error_display() {
        let err = CacheError::DirUnavailable;
        assert_eq!(err.to_string(), "缓存目录不可用");
    }

    #[test]
    fn test_commit_tmp_error() {
        let err = CacheError::CommitTmp("重命名失败".to_string());
        assert!(err.to_string().contains("重命名失败"));
    }
}
