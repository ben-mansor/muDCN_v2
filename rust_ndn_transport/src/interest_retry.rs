// μDCN Interest Retry Strategy Implementation
//
// This module implements retry strategies for Interest packet transmission
// with exponential backoff and configurable policies.

use std::time::Duration;
use rand::Rng;

use crate::error::{Error, Result};

/// RetryPolicy defines parameters for Interest retransmission attempts
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retry attempts
    pub max_attempts: u32,
    
    /// Base delay between retries in milliseconds
    pub base_delay_ms: u64,
    
    /// Maximum delay between retries in milliseconds
    pub max_delay_ms: u64,
    
    /// Backoff multiplier (delay grows by this factor each attempt)
    pub backoff_factor: f64,
    
    /// Whether to add jitter to retry delays
    pub with_jitter: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay_ms: 100,
            max_delay_ms: 5000,
            backoff_factor: 2.0,
            with_jitter: true,
        }
    }
}

impl RetryPolicy {
    /// Creates a policy for quick retries with short intervals
    pub fn quick_retries() -> Self {
        Self {
            max_attempts: 5,
            base_delay_ms: 50,
            max_delay_ms: 1000,
            backoff_factor: 1.5,
            with_jitter: true,
        }
    }
    
    /// Creates a policy for slow but persistent retries
    pub fn persistent_retries() -> Self {
        Self {
            max_attempts: 10,
            base_delay_ms: 500,
            max_delay_ms: 30000,
            backoff_factor: 2.0,
            with_jitter: true,
        }
    }
    
    /// Calculate the delay for a specific retry attempt
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::from_millis(0);
        }
        
        // Calculate base exponential backoff
        let exponential_delay = (self.base_delay_ms as f64 * self.backoff_factor.powf((attempt - 1) as f64)) as u64;
        
        // Apply maximum delay cap
        let capped_delay = std::cmp::min(exponential_delay, self.max_delay_ms);
        
        // Add jitter if configured
        let final_delay = if self.with_jitter {
            // Add random jitter of up to 25%
            let jitter = rand::thread_rng().gen_range(0.75..1.25);
            (capped_delay as f64 * jitter) as u64
        } else {
            capped_delay
        };
        
        Duration::from_millis(final_delay)
    }
    
    /// Should we retry based on the error and attempt number
    pub fn should_retry(&self, error: &Error, attempt: u32) -> bool {
        // Don't retry if we've reached max attempts
        if attempt >= self.max_attempts {
            return false;
        }
        
        // Determine if the error is retryable
        match error {
            // Network errors are generally retryable
            Error::IoError(_) => true,
            Error::ConnectionError(_) => true,
            Error::Timeout(_) => true,
            
            // Protocol errors are not retryable
            Error::ParsingError(_) => false,
            Error::InvalidArgument(_) => false,
            
            // Other errors may be retryable
            Error::Other(msg) => {
                // Check for specific error messages that might be retryable
                msg.contains("temporary") || 
                msg.contains("timeout") || 
                msg.contains("reset") ||
                msg.contains("connection")
            }
            
            // By default, don't retry for unknown error types
            _ => false,
        }
    }
}

/// Execute a function with retry according to the provided policy
pub async fn with_retry<T, F, Fut>(
    operation: F, 
    policy: &RetryPolicy,
    operation_name: &str
) -> Result<T> 
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut attempt = 0;
    let mut last_error = None;
    
    loop {
        attempt += 1;
        
        // Try the operation
        match operation().await {
            Ok(result) => {
                return Ok(result);
            }
            Err(error) => {
                // Determine if we should retry
                if policy.should_retry(&error, attempt) {
                    // Calculate retry delay
                    let delay = policy.delay_for_attempt(attempt);
                    
                    tracing::warn!(
                        "Operation '{}' failed (attempt {}/{}): {}. Retrying after {:?}",
                        operation_name,
                        attempt,
                        policy.max_attempts,
                        error,
                        delay
                    );
                    
                    // Wait before retrying
                    tokio::time::sleep(delay).await;
                    last_error = Some(error);
                } else {
                    // No more retries, return the error
                    return Err(error);
                }
            }
        }
    }
    
    // This should be unreachable, but if we get here, return the last error
    Err(last_error.unwrap_or_else(|| Error::Other("Unknown error during retry".to_string())))
}
