//! 重试策略 — 指数退避 + 抖动 + Retry-After 头部尊重。
//!
//! 与 [`claw-code-api`] `providers::anthropic` 内联的退避逻辑相比，本模块作了
//! 两处改进（[ADR-118] §8.5 顺势处理）：
//!
//! 1. **独立模块**：claw-code 把退避当成 `AnthropicClient` 私有方法。本 crate
//!    把 `RetryPolicy` 提升为公共类型，PR-A.3 `OpenAiCompatClient` 共享同一份
//!    实现，避免重复代码。
//! 2. **尊重 `Retry-After`**：claw-code 完全忽略 `Retry-After` 头部，固定走
//!    指数退避。我们在 [`RetryPolicy::delay_for_attempt`] 中接受可选 `retry_after`
//!    参数，优先采纳上游建议（取 `min(retry_after, max_backoff)` 防恶意头）。
//!
//! [`claw-code-api`]: https://github.com/charmbracelet/claw-code
//! [ADR-118]: ../docs/plans/architecture-refactor/adr-118-claw-code-readonly-and-self-impl.md

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 指数退避重试策略。
///
/// 默认值：max_retries=8，initial_backoff=1s，max_backoff=128s（与 claw-code 对齐）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    /// 最大重试次数（不含首次请求）。
    pub max_retries: u32,
    /// 第一次重试前的等待。后续重试在此基础上指数翻倍（饱和到 `max_backoff`）。
    pub initial_backoff: Duration,
    /// 退避上限；任何计算出的延迟都不会超过此值。
    pub max_backoff: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 8,
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(128),
        }
    }
}

impl RetryPolicy {
    /// 计算第 `attempt` 次重试的基础退避（指数 + 上限封顶）。
    ///
    /// `attempt` 从 1 开始计（第一次重试 = attempt=1）。返回 `None` 当移位
    /// 溢出（极端 `attempt` 值），调用方应直接放弃重试。
    #[must_use]
    pub fn backoff_for_attempt(&self, attempt: u32) -> Option<Duration> {
        let exp = attempt.saturating_sub(1);
        let multiplier = 1_u32.checked_shl(exp)?;
        Some(
            self.initial_backoff
                .checked_mul(multiplier)
                .map_or(self.max_backoff, |delay| delay.min(self.max_backoff)),
        )
    }

    /// 计算最终延迟：当 `retry_after` 提供时优先使用（同样受 `max_backoff` 约束），
    /// 否则走指数退避 + 抖动。
    ///
    /// 抖动是 `[0, base]` 区间的加法抖动 —— 不会缩短延迟，只会拉长到至多 2×base。
    #[must_use]
    pub fn delay_for_attempt(
        &self,
        attempt: u32,
        retry_after: Option<Duration>,
    ) -> Option<Duration> {
        if let Some(retry_after) = retry_after {
            return Some(retry_after.min(self.max_backoff));
        }
        let base = self.backoff_for_attempt(attempt)?;
        Some(base + jitter_for_base(base))
    }
}

/// 解析 HTTP `Retry-After` 头部。
///
/// 仅支持 delta-seconds 整数格式（最常见）。HTTP-date 格式不解析（claw-code 也未
/// 实现，留给未来需要时扩展）。
#[must_use]
pub fn parse_retry_after(value: &str) -> Option<Duration> {
    value.trim().parse::<u64>().ok().map(Duration::from_secs)
}

static JITTER_COUNTER: AtomicU64 = AtomicU64::new(0);

/// 加法抖动 — 在 `[0, base]` 区间内均匀采样。
///
/// 熵源：纳秒级 wall clock + 进程 atomic 计数器，经 splitmix64 finalizer 混淆。
/// 不需要密码学强度（只用于退避去关联）。
fn jitter_for_base(base: Duration) -> Duration {
    let base_nanos = u64::try_from(base.as_nanos()).unwrap_or(u64::MAX);
    if base_nanos == 0 {
        return Duration::ZERO;
    }
    let raw_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX))
        .unwrap_or(0);
    let tick = JITTER_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut mixed = raw_nanos
        .wrapping_add(tick)
        .wrapping_add(0x9E37_79B9_7F4A_7C15);
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    mixed ^= mixed >> 31;
    let jitter_nanos = mixed % base_nanos.saturating_add(1);
    Duration::from_nanos(jitter_nanos)
}

#[cfg(test)]
mod tests {
    use super::{RetryPolicy, parse_retry_after};
    use std::time::Duration;

    #[test]
    fn default_policy_matches_claw_code_constants() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_retries, 8);
        assert_eq!(policy.initial_backoff, Duration::from_secs(1));
        assert_eq!(policy.max_backoff, Duration::from_secs(128));
    }

    #[test]
    fn backoff_doubles_per_attempt_until_max() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.backoff_for_attempt(1), Some(Duration::from_secs(1)));
        assert_eq!(policy.backoff_for_attempt(2), Some(Duration::from_secs(2)));
        assert_eq!(policy.backoff_for_attempt(3), Some(Duration::from_secs(4)));
        assert_eq!(
            policy.backoff_for_attempt(8),
            Some(Duration::from_secs(128))
        );
        // attempts beyond saturate at max_backoff (no overflow)
        assert_eq!(
            policy.backoff_for_attempt(20),
            Some(Duration::from_secs(128))
        );
    }

    #[test]
    fn backoff_saturates_at_max_when_initial_is_large() {
        let policy = RetryPolicy {
            max_retries: 3,
            initial_backoff: Duration::from_secs(100),
            max_backoff: Duration::from_secs(120),
        };
        assert_eq!(
            policy.backoff_for_attempt(1),
            Some(Duration::from_secs(100))
        );
        assert_eq!(
            policy.backoff_for_attempt(2),
            Some(Duration::from_secs(120))
        );
        assert_eq!(
            policy.backoff_for_attempt(3),
            Some(Duration::from_secs(120))
        );
    }

    #[test]
    fn delay_uses_retry_after_when_present_capped_by_max_backoff() {
        let policy = RetryPolicy::default();
        let suggested = Some(Duration::from_secs(10));
        let delay = policy
            .delay_for_attempt(1, suggested)
            .expect("delay computable");
        assert_eq!(delay, Duration::from_secs(10));

        // Server-suggested value beyond max_backoff should clamp.
        let huge = Some(Duration::from_secs(10_000));
        let clamped = policy.delay_for_attempt(1, huge).expect("delay computable");
        assert_eq!(clamped, Duration::from_secs(128));
    }

    #[test]
    fn delay_falls_back_to_jittered_exponential_when_no_retry_after() {
        let policy = RetryPolicy::default();
        let base = policy.backoff_for_attempt(2).expect("base attempt 2");
        let delay = policy.delay_for_attempt(2, None).expect("delay computable");
        assert!(
            delay >= base && delay <= base * 2,
            "delay {delay:?} not within [{base:?}, 2*{base:?}]"
        );
    }

    #[test]
    fn parse_retry_after_accepts_integer_seconds() {
        assert_eq!(parse_retry_after("3"), Some(Duration::from_secs(3)));
        assert_eq!(parse_retry_after("  120  "), Some(Duration::from_secs(120)));
        assert_eq!(parse_retry_after("0"), Some(Duration::ZERO));
    }

    #[test]
    fn parse_retry_after_rejects_non_integer_formats() {
        // HTTP-date format is not supported (claw-code parity).
        assert!(parse_retry_after("Wed, 21 Oct 2026 07:28:00 GMT").is_none());
        assert!(parse_retry_after("3.5").is_none());
        assert!(parse_retry_after("").is_none());
        assert!(parse_retry_after("-1").is_none());
    }
}
