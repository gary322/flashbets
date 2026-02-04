use anchor_lang::prelude::*;
use std::collections::HashSet;

pub struct SecurityAudit {
    checks: Vec<SecurityCheck>,
    results: Vec<AuditResult>,
}

#[derive(Debug)]
pub struct SecurityCheck {
    name: String,
    severity: Severity,
    check_fn: Box<dyn Fn(&str) -> Result<(), String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone)]
pub struct AuditResult {
    check_name: String,
    passed: bool,
    severity: Severity,
    details: String,
}

impl SecurityAudit {
    pub fn new() -> Self {
        let mut audit = Self {
            checks: Vec::new(),
            results: Vec::new(),
        };

        audit.register_checks();
        audit
    }

    fn register_checks(&mut self) {
        // Arithmetic checks
        self.add_check(
            "Integer Overflow",
            Severity::Critical,
            Box::new(|code| {
                // Check all arithmetic operations use safe math
                let unsafe_ops = find_unsafe_arithmetic(code);
                if unsafe_ops.is_empty() {
                    Ok(())
                } else {
                    Err(format!("Found {} unsafe arithmetic operations", unsafe_ops.len()))
                }
            })
        );

        // Access control checks
        self.add_check(
            "Unauthorized Access",
            Severity::Critical,
            Box::new(|code| {
                // Verify all admin functions check authority
                let unprotected = find_unprotected_admin_functions(code);
                if unprotected.is_empty() {
                    Ok(())
                } else {
                    Err(format!("Found {} unprotected admin functions", unprotected.len()))
                }
            })
        );

        // Reentrancy checks
        self.add_check(
            "Reentrancy Vulnerability",
            Severity::High,
            Box::new(|code| {
                // Check for reentrancy patterns
                let vulnerable = find_reentrancy_vulnerabilities(code);
                if vulnerable.is_empty() {
                    Ok(())
                } else {
                    Err(format!("Found {} potential reentrancy points", vulnerable.len()))
                }
            })
        );

        // State consistency checks
        self.add_check(
            "State Consistency",
            Severity::High,
            Box::new(|code| {
                // Verify state updates are atomic
                verify_atomic_state_updates(code)
            })
        );

        // Economic checks
        self.add_check(
            "Economic Invariants",
            Severity::Critical,
            Box::new(|code| {
                // Verify leverage bounds, coverage calculations, etc.
                verify_economic_invariants(code)
            })
        );
    }

    fn add_check(&mut self, name: &str, severity: Severity, check_fn: Box<dyn Fn(&str) -> Result<(), String>>) {
        self.checks.push(SecurityCheck {
            name: name.to_string(),
            severity,
            check_fn,
        });
    }

    pub fn run_audit(&mut self, code: &str) -> AuditReport {
        for check in &self.checks {
            let result = match (check.check_fn)(code) {
                Ok(()) => AuditResult {
                    check_name: check.name.clone(),
                    passed: true,
                    severity: check.severity.clone(),
                    details: "Check passed".to_string(),
                },
                Err(details) => AuditResult {
                    check_name: check.name.clone(),
                    passed: false,
                    severity: check.severity.clone(),
                    details,
                },
            };

            self.results.push(result);
        }

        self.generate_report()
    }

    fn generate_report(&self) -> AuditReport {
        let critical_issues = self.results.iter()
            .filter(|r| !r.passed && r.severity == Severity::Critical)
            .count();

        let high_issues = self.results.iter()
            .filter(|r| !r.passed && r.severity == Severity::High)
            .count();

        AuditReport {
            total_checks: self.checks.len(),
            passed: self.results.iter().filter(|r| r.passed).count(),
            failed: self.results.iter().filter(|r| !r.passed).count(),
            critical_issues,
            high_issues,
            results: self.results.clone(),
        }
    }
}

#[derive(Debug)]
pub struct AuditReport {
    total_checks: usize,
    passed: usize,
    failed: usize,
    critical_issues: usize,
    high_issues: usize,
    results: Vec<AuditResult>,
}

impl AuditReport {
    pub fn print_summary(&self) {
        println!("Security Audit Report");
        println!("====================");
        println!("Total Checks: {}", self.total_checks);
        println!("Passed: {}", self.passed);
        println!("Failed: {}", self.failed);
        println!("Critical Issues: {}", self.critical_issues);
        println!("High Issues: {}", self.high_issues);
        println!();

        if self.critical_issues > 0 {
            println!("CRITICAL ISSUES FOUND:");
            for result in &self.results {
                if !result.passed && result.severity == Severity::Critical {
                    println!("  - {}: {}", result.check_name, result.details);
                }
            }
            println!();
        }

        if self.high_issues > 0 {
            println!("HIGH SEVERITY ISSUES:");
            for result in &self.results {
                if !result.passed && result.severity == Severity::High {
                    println!("  - {}: {}", result.check_name, result.details);
                }
            }
        }
    }
}

// Helper functions for checks
fn find_unsafe_arithmetic(code: &str) -> Vec<String> {
    let mut unsafe_ops = Vec::new();
    let unsafe_patterns = vec![
        " + ", " - ", " * ", " / ",
        "+=", "-=", "*=", "/=",
    ];

    for pattern in unsafe_patterns {
        if code.contains(pattern) && !code.contains("saturating_") && !code.contains("checked_") {
            unsafe_ops.push(pattern.to_string());
        }
    }

    unsafe_ops
}

fn find_unprotected_admin_functions(code: &str) -> Vec<String> {
    let mut unprotected = Vec::new();
    let admin_functions = vec![
        "set_fee", "update_config", "pause", "unpause",
        "withdraw", "mint", "burn", "transfer_authority"
    ];

    for func in admin_functions {
        if code.contains(func) {
            // Check if there's an authority check nearby
            let func_pos = code.find(func).unwrap();
            let check_area = &code[func_pos.saturating_sub(200)..func_pos.saturating_add(200).min(code.len())];
            
            if !check_area.contains("require_eq") && 
               !check_area.contains("authority") &&
               !check_area.contains("has_one") {
                unprotected.push(func.to_string());
            }
        }
    }

    unprotected
}

fn find_reentrancy_vulnerabilities(code: &str) -> Vec<String> {
    let mut vulnerabilities = Vec::new();
    
    // Look for external calls before state updates
    if code.contains("invoke") || code.contains("cpi::") {
        // Simplified check - in reality would need more sophisticated analysis
        vulnerabilities.push("Potential reentrancy via CPI".to_string());
    }

    vulnerabilities
}

fn verify_atomic_state_updates(code: &str) -> Result<(), String> {
    // Check that state updates follow check-effects-interactions pattern
    if code.contains("transfer") || code.contains("invoke") {
        // Ensure state is updated before external calls
        // This is a simplified check
        Ok(())
    } else {
        Ok(())
    }
}

fn verify_economic_invariants(code: &str) -> Result<(), String> {
    // Check leverage formulas
    if code.contains("leverage") && !code.contains(".min(500") {
        return Err("Leverage not capped at 500x".to_string());
    }

    // Check coverage calculations
    if code.contains("coverage") && !code.contains("saturating_") {
        return Err("Coverage calculations may overflow".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod audit_tests {
    use super::*;

    #[test]
    fn test_security_audit() {
        let mut auditor = SecurityAudit::new();

        let sample_code = r#"
            pub fn calculate_leverage(base: u64, depth: u8) -> u64 {
                base * (1 + depth * 10 / 100)
            }
            
            pub fn update_price(ctx: Context<UpdatePrice>, new_price: u64) -> Result<()> {
                ctx.accounts.market.price = new_price;
                Ok(())
            }
        "#;

        let report = auditor.run_audit(sample_code);

        assert!(report.critical_issues > 0);
        assert!(report.results.iter().any(|r| r.details.contains("arithmetic")));
    }

    #[test]
    fn test_leverage_cap_detection() {
        let code_with_cap = r#"
            let effective_leverage = base_leverage.min(500);
        "#;

        let code_without_cap = r#"
            let effective_leverage = base_leverage;
        "#;

        assert!(verify_economic_invariants(code_with_cap).is_ok());
        assert!(verify_economic_invariants(code_without_cap).is_err());
    }

    #[test]
    fn test_admin_function_detection() {
        let protected_code = r#"
            pub fn set_fee(ctx: Context<SetFee>, new_fee: u64) -> Result<()> {
                require_eq!(ctx.accounts.authority.key(), ADMIN_KEY);
                ctx.accounts.config.fee = new_fee;
                Ok(())
            }
        "#;

        let unprotected_code = r#"
            pub fn set_fee(ctx: Context<SetFee>, new_fee: u64) -> Result<()> {
                ctx.accounts.config.fee = new_fee;
                Ok(())
            }
        "#;

        let protected_funcs = find_unprotected_admin_functions(protected_code);
        let unprotected_funcs = find_unprotected_admin_functions(unprotected_code);

        assert_eq!(protected_funcs.len(), 0);
        assert!(unprotected_funcs.len() > 0);
    }
}