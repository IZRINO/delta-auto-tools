//! 单制作台试运行失败结果。

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CraftTrialFailure {
    pub step: String,
    pub message: String,
    pub requires_uncertain: bool,
}

impl CraftTrialFailure {
    pub(crate) fn is_isolated(&self) -> bool {
        self.step == "craft.isolated"
    }
}
