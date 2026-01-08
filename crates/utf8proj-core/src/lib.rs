//! # utf8proj-core
//!
//! Core domain model and traits for the utf8proj scheduling engine.
//!
//! This crate provides:
//! - Domain types: `Project`, `Task`, `Resource`, `Calendar`, `Schedule`
//! - Core traits: `Scheduler`, `WhatIfAnalysis`, `Renderer`
//! - Error types and result aliases
//!
//! ## Example
//!
//! ```rust
//! use utf8proj_core::{Project, Task, Resource, Duration};
//!
//! let mut project = Project::new("My Project");
//! project.tasks.push(
//!     Task::new("design")
//!         .effort(Duration::days(5))
//!         .assign("dev")
//! );
//! project.tasks.push(
//!     Task::new("implement")
//!         .effort(Duration::days(10))
//!         .depends_on("design")
//!         .assign("dev")
//! );
//! project.resources.push(Resource::new("dev").capacity(1.0));
//! ```

use chrono::{Datelike, NaiveDate};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

// ============================================================================
// Type Aliases
// ============================================================================

/// Unique identifier for a task
pub type TaskId = String;

/// Unique identifier for a resource
pub type ResourceId = String;

/// Unique identifier for a calendar
pub type CalendarId = String;

/// Unique identifier for a resource profile
pub type ProfileId = String;

/// Unique identifier for a trait
pub type TraitId = String;

/// Duration in working time
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Duration {
    /// Number of minutes
    pub minutes: i64,
}

impl Duration {
    pub const fn zero() -> Self {
        Self { minutes: 0 }
    }

    pub const fn minutes(m: i64) -> Self {
        Self { minutes: m }
    }

    pub const fn hours(h: i64) -> Self {
        Self { minutes: h * 60 }
    }

    pub const fn days(d: i64) -> Self {
        Self { minutes: d * 8 * 60 } // 8-hour workday
    }

    pub const fn weeks(w: i64) -> Self {
        Self { minutes: w * 5 * 8 * 60 } // 5-day workweek
    }

    pub fn as_days(&self) -> f64 {
        self.minutes as f64 / (8.0 * 60.0)
    }

    pub fn as_hours(&self) -> f64 {
        self.minutes as f64 / 60.0
    }
}

impl std::ops::Add for Duration {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self { minutes: self.minutes + rhs.minutes }
    }
}

impl std::ops::Sub for Duration {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self { minutes: self.minutes - rhs.minutes }
    }
}

/// Monetary amount with currency
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money {
    pub amount: Decimal,
    pub currency: String,
}

impl Money {
    pub fn new(amount: impl Into<Decimal>, currency: impl Into<String>) -> Self {
        Self {
            amount: amount.into(),
            currency: currency.into(),
        }
    }
}

// ============================================================================
// RFC-0001: Progressive Resource Refinement Types
// ============================================================================

/// A trait that modifies resource rates (RFC-0001)
///
/// Traits are scalar modifiers, not behavioral mixins. They apply
/// multiplicative rate adjustments (e.g., senior = 1.3x, junior = 0.8x).
///
/// # Example
///
/// ```rust
/// use utf8proj_core::Trait;
///
/// let senior = Trait::new("senior")
///     .description("5+ years experience")
///     .rate_multiplier(1.3);
///
/// assert_eq!(senior.rate_multiplier, 1.3);
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Trait {
    /// Unique identifier
    pub id: TraitId,
    /// Human-readable name
    pub name: String,
    /// Optional description
    pub description: Option<String>,
    /// Rate multiplier (1.0 = no change, 1.3 = 30% increase)
    pub rate_multiplier: f64,
}

impl Trait {
    /// Create a new trait with the given ID
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            name: id.clone(),
            id,
            description: None,
            rate_multiplier: 1.0,
        }
    }

    /// Set the trait description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set the rate multiplier
    pub fn rate_multiplier(mut self, multiplier: f64) -> Self {
        self.rate_multiplier = multiplier;
        self
    }
}

/// Rate range for abstract resource profiles (RFC-0001)
///
/// Represents a cost range with min/max bounds and optional currency.
/// Used during early planning when exact rates are unknown.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RateRange {
    /// Minimum rate (best case)
    pub min: Decimal,
    /// Maximum rate (worst case)
    pub max: Decimal,
    /// Currency (defaults to project currency if not set)
    pub currency: Option<String>,
}

impl RateRange {
    /// Create a new rate range
    pub fn new(min: impl Into<Decimal>, max: impl Into<Decimal>) -> Self {
        Self {
            min: min.into(),
            max: max.into(),
            currency: None,
        }
    }

    /// Set the currency
    pub fn currency(mut self, currency: impl Into<String>) -> Self {
        self.currency = Some(currency.into());
        self
    }

    /// Calculate the expected (midpoint) rate
    pub fn expected(&self) -> Decimal {
        (self.min + self.max) / Decimal::from(2)
    }

    /// Calculate the spread percentage: (max - min) / expected * 100
    pub fn spread_percent(&self) -> f64 {
        let expected = self.expected();
        if expected.is_zero() {
            return 0.0;
        }
        let spread = self.max - self.min;
        // Convert to f64 for percentage calculation
        use rust_decimal::prelude::ToPrimitive;
        (spread / expected).to_f64().unwrap_or(0.0) * 100.0
    }

    /// Check if this is a collapsed range (min == max)
    pub fn is_collapsed(&self) -> bool {
        self.min == self.max
    }

    /// Check if the range is inverted (min > max)
    pub fn is_inverted(&self) -> bool {
        self.min > self.max
    }

    /// Apply a multiplier to the range (for trait composition)
    pub fn apply_multiplier(&self, multiplier: f64) -> Self {
        use rust_decimal::prelude::FromPrimitive;
        let mult = Decimal::from_f64(multiplier).unwrap_or(Decimal::ONE);
        Self {
            min: self.min * mult,
            max: self.max * mult,
            currency: self.currency.clone(),
        }
    }
}

/// Resource rate that can be either fixed or a range (RFC-0001)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ResourceRate {
    /// Fixed rate (concrete resource)
    Fixed(Money),
    /// Rate range (abstract profile)
    Range(RateRange),
}

impl ResourceRate {
    /// Get the expected rate value
    pub fn expected(&self) -> Decimal {
        match self {
            ResourceRate::Fixed(money) => money.amount,
            ResourceRate::Range(range) => range.expected(),
        }
    }

    /// Check if this is a range (abstract)
    pub fn is_range(&self) -> bool {
        matches!(self, ResourceRate::Range(_))
    }

    /// Check if this is a fixed rate (concrete)
    pub fn is_fixed(&self) -> bool {
        matches!(self, ResourceRate::Fixed(_))
    }
}

/// Abstract resource profile for planning (RFC-0001)
///
/// Represents a role or capability, not a specific person.
/// Used during early estimation when staffing is not finalized.
///
/// # Example
///
/// ```rust
/// use utf8proj_core::{ResourceProfile, RateRange};
/// use rust_decimal::Decimal;
///
/// let backend_dev = ResourceProfile::new("backend_dev")
///     .name("Backend Developer")
///     .description("Server-side development")
///     .specializes("developer")
///     .skill("java")
///     .skill("sql")
///     .rate_range(RateRange::new(Decimal::from(550), Decimal::from(800)));
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceProfile {
    /// Unique identifier
    pub id: ProfileId,
    /// Human-readable name
    pub name: String,
    /// Optional description
    pub description: Option<String>,
    /// Parent profile (constraint refinement, not OO inheritance)
    pub specializes: Option<ProfileId>,
    /// Required skills
    pub skills: Vec<String>,
    /// Applied traits (rate modifiers)
    pub traits: Vec<TraitId>,
    /// Rate (can be range or fixed)
    pub rate: Option<ResourceRate>,
    /// Custom calendar
    pub calendar: Option<CalendarId>,
    /// Efficiency factor
    pub efficiency: Option<f32>,
}

impl ResourceProfile {
    /// Create a new resource profile with the given ID
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            name: id.clone(),
            id,
            description: None,
            specializes: None,
            skills: Vec::new(),
            traits: Vec::new(),
            rate: None,
            calendar: None,
            efficiency: None,
        }
    }

    /// Set the profile name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set the description
    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set the parent profile (specialization)
    pub fn specializes(mut self, parent: impl Into<String>) -> Self {
        self.specializes = Some(parent.into());
        self
    }

    /// Add a skill
    pub fn skill(mut self, skill: impl Into<String>) -> Self {
        self.skills.push(skill.into());
        self
    }

    /// Add multiple skills
    pub fn skills(mut self, skills: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.skills.extend(skills.into_iter().map(|s| s.into()));
        self
    }

    /// Add a trait
    pub fn with_trait(mut self, trait_id: impl Into<String>) -> Self {
        self.traits.push(trait_id.into());
        self
    }

    /// Add multiple traits
    pub fn with_traits(mut self, traits: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.traits.extend(traits.into_iter().map(|t| t.into()));
        self
    }

    /// Set a rate range
    pub fn rate_range(mut self, range: RateRange) -> Self {
        self.rate = Some(ResourceRate::Range(range));
        self
    }

    /// Set a fixed rate
    pub fn rate(mut self, rate: Money) -> Self {
        self.rate = Some(ResourceRate::Fixed(rate));
        self
    }

    /// Set the calendar
    pub fn calendar(mut self, calendar: impl Into<String>) -> Self {
        self.calendar = Some(calendar.into());
        self
    }

    /// Set the efficiency factor
    pub fn efficiency(mut self, efficiency: f32) -> Self {
        self.efficiency = Some(efficiency);
        self
    }

    /// Check if this profile is abstract (has no fixed rate or is a range)
    pub fn is_abstract(&self) -> bool {
        match &self.rate {
            None => true,
            Some(ResourceRate::Range(_)) => true,
            Some(ResourceRate::Fixed(_)) => false,
        }
    }
}

/// Cost range for scheduled tasks (RFC-0001)
///
/// Represents the computed cost range for a task or project
/// based on abstract resource assignments.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CostRange {
    /// Minimum cost (best case)
    pub min: Decimal,
    /// Expected cost (based on policy)
    pub expected: Decimal,
    /// Maximum cost (worst case)
    pub max: Decimal,
    /// Currency
    pub currency: String,
}

impl CostRange {
    /// Create a new cost range
    pub fn new(min: Decimal, expected: Decimal, max: Decimal, currency: impl Into<String>) -> Self {
        Self {
            min,
            expected,
            max,
            currency: currency.into(),
        }
    }

    /// Create a fixed (zero-spread) cost range
    pub fn fixed(amount: Decimal, currency: impl Into<String>) -> Self {
        Self {
            min: amount,
            expected: amount,
            max: amount,
            currency: currency.into(),
        }
    }

    /// Calculate the spread percentage: ±((max - min) / 2 / expected) * 100
    pub fn spread_percent(&self) -> f64 {
        if self.expected.is_zero() {
            return 0.0;
        }
        let half_spread = (self.max - self.min) / Decimal::from(2);
        use rust_decimal::prelude::ToPrimitive;
        (half_spread / self.expected).to_f64().unwrap_or(0.0) * 100.0
    }

    /// Check if this is a fixed cost (zero spread)
    pub fn is_fixed(&self) -> bool {
        self.min == self.max
    }

    /// Add two cost ranges
    pub fn add(&self, other: &CostRange) -> Self {
        Self {
            min: self.min + other.min,
            expected: self.expected + other.expected,
            max: self.max + other.max,
            currency: self.currency.clone(),
        }
    }
}

/// Policy for calculating expected cost from ranges (RFC-0001)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CostPolicy {
    /// Use midpoint: (min + max) / 2
    #[default]
    Midpoint,
    /// Use minimum (optimistic)
    Optimistic,
    /// Use maximum (pessimistic)
    Pessimistic,
}

impl CostPolicy {
    /// Calculate expected value from a range
    pub fn expected(&self, min: Decimal, max: Decimal) -> Decimal {
        match self {
            CostPolicy::Midpoint => (min + max) / Decimal::from(2),
            CostPolicy::Optimistic => min,
            CostPolicy::Pessimistic => max,
        }
    }
}

// ============================================================================
// Project
// ============================================================================

/// A complete project definition
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    /// Unique identifier
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Project start date
    pub start: NaiveDate,
    /// Project end date (optional, can be computed)
    pub end: Option<NaiveDate>,
    /// Default calendar for the project
    pub calendar: CalendarId,
    /// Currency for cost calculations
    pub currency: String,
    /// All tasks in the project (may be hierarchical)
    pub tasks: Vec<Task>,
    /// All resources available to the project
    pub resources: Vec<Resource>,
    /// Calendar definitions
    pub calendars: Vec<Calendar>,
    /// Scenario definitions (for what-if analysis)
    pub scenarios: Vec<Scenario>,
    /// Custom attributes (timezone, etc.)
    pub attributes: HashMap<String, String>,

    // RFC-0001: Progressive Resource Refinement fields
    /// Resource profiles (abstract roles/capabilities)
    pub profiles: Vec<ResourceProfile>,
    /// Trait definitions (rate modifiers)
    pub traits: Vec<Trait>,
    /// Policy for calculating expected cost from ranges
    pub cost_policy: CostPolicy,
}

impl Project {
    /// Create a new project with the given name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: String::new(),
            name: name.into(),
            start: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            end: None,
            calendar: "default".into(),
            currency: "USD".into(),
            tasks: Vec::new(),
            resources: Vec::new(),
            calendars: vec![Calendar::default()],
            scenarios: Vec::new(),
            attributes: HashMap::new(),
            profiles: Vec::new(),
            traits: Vec::new(),
            cost_policy: CostPolicy::default(),
        }
    }

    /// Get a task by ID (searches recursively)
    pub fn get_task(&self, id: &str) -> Option<&Task> {
        fn find_task<'a>(tasks: &'a [Task], id: &str) -> Option<&'a Task> {
            for task in tasks {
                if task.id == id {
                    return Some(task);
                }
                if let Some(found) = find_task(&task.children, id) {
                    return Some(found);
                }
            }
            None
        }
        find_task(&self.tasks, id)
    }

    /// Get a resource by ID
    pub fn get_resource(&self, id: &str) -> Option<&Resource> {
        self.resources.iter().find(|r| r.id == id)
    }

    /// Get a resource profile by ID (RFC-0001)
    pub fn get_profile(&self, id: &str) -> Option<&ResourceProfile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    /// Get a trait by ID (RFC-0001)
    pub fn get_trait(&self, id: &str) -> Option<&Trait> {
        self.traits.iter().find(|t| t.id == id)
    }

    /// Get all leaf tasks (tasks without children)
    pub fn leaf_tasks(&self) -> Vec<&Task> {
        fn collect_leaves<'a>(tasks: &'a [Task], result: &mut Vec<&'a Task>) {
            for task in tasks {
                if task.children.is_empty() {
                    result.push(task);
                } else {
                    collect_leaves(&task.children, result);
                }
            }
        }
        let mut leaves = Vec::new();
        collect_leaves(&self.tasks, &mut leaves);
        leaves
    }
}

// ============================================================================
// Task
// ============================================================================

/// A schedulable unit of work
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    /// Unique identifier
    pub id: TaskId,
    /// Human-readable description (from quoted string in DSL)
    pub name: String,
    /// Optional short display name (MS Project "Task Name" style)
    pub summary: Option<String>,
    /// Work effort required (person-time)
    pub effort: Option<Duration>,
    /// Calendar duration (overrides effort-based calculation)
    pub duration: Option<Duration>,
    /// Task dependencies
    pub depends: Vec<Dependency>,
    /// Resource assignments
    pub assigned: Vec<ResourceRef>,
    /// Scheduling priority (higher = scheduled first)
    pub priority: u32,
    /// Scheduling constraints
    pub constraints: Vec<TaskConstraint>,
    /// Is this a milestone (zero duration)?
    pub milestone: bool,
    /// Child tasks (WBS hierarchy)
    pub children: Vec<Task>,
    /// Completion percentage (for tracking)
    pub complete: Option<f32>,
    /// Actual start date (when work actually began)
    pub actual_start: Option<NaiveDate>,
    /// Actual finish date (when work actually completed)
    pub actual_finish: Option<NaiveDate>,
    /// Task status for progress tracking
    pub status: Option<TaskStatus>,
    /// Custom attributes
    pub attributes: HashMap<String, String>,
}

impl Task {
    /// Create a new task with the given ID
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            name: id.clone(),
            id,
            summary: None,
            effort: None,
            duration: None,
            depends: Vec::new(),
            assigned: Vec::new(),
            priority: 500,
            constraints: Vec::new(),
            milestone: false,
            children: Vec::new(),
            complete: None,
            actual_start: None,
            actual_finish: None,
            status: None,
            attributes: HashMap::new(),
        }
    }

    /// Set the task name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set the task summary (short display name)
    pub fn summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    /// Set the effort
    pub fn effort(mut self, effort: Duration) -> Self {
        self.effort = Some(effort);
        self
    }

    /// Set the duration
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }

    /// Add a dependency (FinishToStart by default)
    pub fn depends_on(mut self, predecessor: impl Into<String>) -> Self {
        self.depends.push(Dependency {
            predecessor: predecessor.into(),
            dep_type: DependencyType::FinishToStart,
            lag: None,
        });
        self
    }

    /// Add a dependency with full control over type and lag
    pub fn with_dependency(mut self, dep: Dependency) -> Self {
        self.depends.push(dep);
        self
    }

    /// Assign a resource
    pub fn assign(mut self, resource: impl Into<String>) -> Self {
        self.assigned.push(ResourceRef {
            resource_id: resource.into(),
            units: 1.0,
        });
        self
    }

    /// Assign a resource with specific allocation units
    ///
    /// Units represent allocation percentage: 1.0 = 100%, 0.5 = 50%, etc.
    /// This affects effort-driven duration calculation:
    ///   Duration = Effort / Total_Units
    pub fn assign_with_units(mut self, resource: impl Into<String>, units: f32) -> Self {
        self.assigned.push(ResourceRef {
            resource_id: resource.into(),
            units,
        });
        self
    }

    /// Set priority
    pub fn priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }

    /// Mark as milestone
    pub fn milestone(mut self) -> Self {
        self.milestone = true;
        self.duration = Some(Duration::zero());
        self
    }

    /// Add a child task
    pub fn child(mut self, child: Task) -> Self {
        self.children.push(child);
        self
    }

    /// Add a temporal constraint
    pub fn constraint(mut self, constraint: TaskConstraint) -> Self {
        self.constraints.push(constraint);
        self
    }

    /// Check if this is a summary task (has children)
    pub fn is_summary(&self) -> bool {
        !self.children.is_empty()
    }

    // ========================================================================
    // Progress Tracking Methods
    // ========================================================================

    /// Calculate remaining duration based on completion percentage.
    /// Uses linear interpolation: remaining = original × (1 - complete/100)
    pub fn remaining_duration(&self) -> Duration {
        let original = self.duration.or(self.effort).unwrap_or(Duration::zero());
        let pct = self.effective_percent_complete() as f64;
        let remaining_minutes = (original.minutes as f64 * (1.0 - pct / 100.0)).round() as i64;
        Duration::minutes(remaining_minutes.max(0))
    }

    /// Get effective completion percentage as u8 (0-100).
    /// Returns 0 if not set, clamped to 0-100 range.
    pub fn effective_percent_complete(&self) -> u8 {
        self.complete
            .map(|c| c.clamp(0.0, 100.0) as u8)
            .unwrap_or(0)
    }

    /// Derive task status from actual dates and completion.
    /// Returns explicit status if set, otherwise derives from data.
    /// For containers, uses effective_progress() to derive status from children.
    pub fn derived_status(&self) -> TaskStatus {
        // Use explicit status if set
        if let Some(ref status) = self.status {
            return status.clone();
        }

        // Derive from actual data - use effective_progress for container rollup
        let pct = self.effective_progress();
        if pct >= 100 || self.actual_finish.is_some() {
            TaskStatus::Complete
        } else if pct > 0 || self.actual_start.is_some() {
            TaskStatus::InProgress
        } else {
            TaskStatus::NotStarted
        }
    }

    /// Set the completion percentage (builder pattern)
    pub fn complete(mut self, pct: f32) -> Self {
        self.complete = Some(pct);
        self
    }

    /// Check if this task is a container (has children)
    pub fn is_container(&self) -> bool {
        !self.children.is_empty()
    }

    /// Calculate container progress as weighted average of children by duration.
    /// Returns None if not a container or if no children have duration.
    /// Formula: Σ(child.percent_complete × child.duration) / Σ(child.duration)
    pub fn container_progress(&self) -> Option<u8> {
        if self.children.is_empty() {
            return None;
        }

        let (total_weighted, total_duration) = self.calculate_weighted_progress();

        if total_duration == 0 {
            return None;
        }

        Some((total_weighted as f64 / total_duration as f64).round() as u8)
    }

    /// Helper to recursively calculate weighted progress from all descendants.
    /// Returns (weighted_sum, total_duration_minutes)
    fn calculate_weighted_progress(&self) -> (i64, i64) {
        let mut total_weighted: i64 = 0;
        let mut total_duration: i64 = 0;

        for child in &self.children {
            if child.is_container() {
                // Recursively get progress from nested containers
                let (child_weighted, child_duration) = child.calculate_weighted_progress();
                total_weighted += child_weighted;
                total_duration += child_duration;
            } else {
                // Leaf task - use its duration and progress
                let duration = child.duration.or(child.effort).unwrap_or(Duration::zero());
                let duration_mins = duration.minutes;
                let pct = child.effective_percent_complete() as i64;

                total_weighted += pct * duration_mins;
                total_duration += duration_mins;
            }
        }

        (total_weighted, total_duration)
    }

    /// Get the effective progress for this task, considering container rollup.
    /// For containers: returns derived progress from children (unless manually overridden).
    /// For leaf tasks: returns the explicit completion percentage.
    pub fn effective_progress(&self) -> u8 {
        // If manual override is set, use it
        if let Some(pct) = self.complete {
            return pct.clamp(0.0, 100.0) as u8;
        }

        // For containers, derive from children
        if let Some(derived) = self.container_progress() {
            return derived;
        }

        // Default to 0
        0
    }

    /// Check if container progress significantly differs from manual override.
    /// Returns Some((manual, derived)) if mismatch > threshold, None otherwise.
    pub fn progress_mismatch(&self, threshold: u8) -> Option<(u8, u8)> {
        if !self.is_container() {
            return None;
        }

        let manual = self.complete.map(|c| c.clamp(0.0, 100.0) as u8)?;
        let derived = self.container_progress()?;

        let diff = (manual as i16 - derived as i16).unsigned_abs() as u8;
        if diff > threshold {
            Some((manual, derived))
        } else {
            None
        }
    }

    /// Set the actual start date (builder pattern)
    pub fn actual_start(mut self, date: NaiveDate) -> Self {
        self.actual_start = Some(date);
        self
    }

    /// Set the actual finish date (builder pattern)
    pub fn actual_finish(mut self, date: NaiveDate) -> Self {
        self.actual_finish = Some(date);
        self
    }

    /// Set the task status (builder pattern)
    pub fn with_status(mut self, status: TaskStatus) -> Self {
        self.status = Some(status);
        self
    }
}

/// Task dependency with type and lag
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dependency {
    /// ID of the predecessor task
    pub predecessor: TaskId,
    /// Type of dependency
    pub dep_type: DependencyType,
    /// Lag time (positive) or lead time (negative)
    pub lag: Option<Duration>,
}

/// Types of task dependencies
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencyType {
    /// Finish-to-Start: successor starts after predecessor finishes
    #[default]
    FinishToStart,
    /// Start-to-Start: successor starts when predecessor starts
    StartToStart,
    /// Finish-to-Finish: successor finishes when predecessor finishes
    FinishToFinish,
    /// Start-to-Finish: successor finishes when predecessor starts
    StartToFinish,
}

/// Task status for progress tracking
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    #[default]
    NotStarted,
    InProgress,
    Complete,
    Blocked,
    AtRisk,
    OnHold,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::NotStarted => write!(f, "Not Started"),
            TaskStatus::InProgress => write!(f, "In Progress"),
            TaskStatus::Complete => write!(f, "Complete"),
            TaskStatus::Blocked => write!(f, "Blocked"),
            TaskStatus::AtRisk => write!(f, "At Risk"),
            TaskStatus::OnHold => write!(f, "On Hold"),
        }
    }
}

/// Scheduling mode classification for capability awareness
///
/// This describes *what kind of schedule* a project represents, not whether
/// it's correct or complete. All modes are valid and appropriate for different
/// planning contexts.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SchedulingMode {
    /// Tasks use `duration:` only, no effort or resource assignments
    /// Suitable for: roadmaps, timelines, regulatory deadlines, migration plans
    /// Capabilities: timeline ✓, utilization ✗, cost tracking ✗
    #[default]
    DurationBased,
    /// Tasks use `effort:` with resource assignments
    /// Suitable for: project planning with team workload tracking
    /// Capabilities: timeline ✓, utilization ✓, cost tracking depends on rates
    EffortBased,
    /// Tasks use `effort:` with resource assignments AND resources have rates
    /// Suitable for: full project management with budget tracking
    /// Capabilities: timeline ✓, utilization ✓, cost tracking ✓
    ResourceLoaded,
}

impl SchedulingMode {
    /// Human-readable description for diagnostics
    pub fn description(&self) -> &'static str {
        match self {
            SchedulingMode::DurationBased => "duration-based (no effort tracking)",
            SchedulingMode::EffortBased => "effort-based (no cost tracking)",
            SchedulingMode::ResourceLoaded => "resource-loaded (full tracking)",
        }
    }

    /// What capabilities are available in this mode
    pub fn capabilities(&self) -> SchedulingCapabilities {
        match self {
            SchedulingMode::DurationBased => SchedulingCapabilities {
                timeline: true,
                utilization: false,
                cost_tracking: false,
            },
            SchedulingMode::EffortBased => SchedulingCapabilities {
                timeline: true,
                utilization: true,
                cost_tracking: false,
            },
            SchedulingMode::ResourceLoaded => SchedulingCapabilities {
                timeline: true,
                utilization: true,
                cost_tracking: true,
            },
        }
    }
}

impl std::fmt::Display for SchedulingMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description())
    }
}

/// Capabilities available for a given scheduling mode
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SchedulingCapabilities {
    /// Can calculate task start/end dates
    pub timeline: bool,
    /// Can track resource utilization
    pub utilization: bool,
    /// Can track project costs
    pub cost_tracking: bool,
}

/// Reference to a resource with allocation units
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceRef {
    /// ID of the resource
    pub resource_id: ResourceId,
    /// Allocation units (1.0 = 100%)
    pub units: f32,
}

/// Constraint on task scheduling
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TaskConstraint {
    /// Task must start on this date
    MustStartOn(NaiveDate),
    /// Task must finish on this date
    MustFinishOn(NaiveDate),
    /// Task cannot start before this date
    StartNoEarlierThan(NaiveDate),
    /// Task must start by this date
    StartNoLaterThan(NaiveDate),
    /// Task cannot finish before this date
    FinishNoEarlierThan(NaiveDate),
    /// Task must finish by this date
    FinishNoLaterThan(NaiveDate),
}

// ============================================================================
// Resource
// ============================================================================

/// A person or equipment that can be assigned to tasks
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Resource {
    /// Unique identifier
    pub id: ResourceId,
    /// Human-readable name
    pub name: String,
    /// Cost rate (per time unit)
    pub rate: Option<Money>,
    /// Capacity (1.0 = full time, 0.5 = half time)
    pub capacity: f32,
    /// Custom calendar (overrides project default)
    pub calendar: Option<CalendarId>,
    /// Efficiency factor (default 1.0)
    pub efficiency: f32,
    /// Custom attributes
    pub attributes: HashMap<String, String>,

    // RFC-0001: Progressive Resource Refinement fields
    /// Profile that this resource specializes (constraint refinement)
    pub specializes: Option<ProfileId>,
    /// Availability (0.0-1.0, multiplied with calendar hours)
    /// Separate from capacity for progressive refinement semantics
    pub availability: Option<f32>,
}

impl Resource {
    /// Create a new resource with the given ID
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            name: id.clone(),
            id,
            rate: None,
            capacity: 1.0,
            calendar: None,
            efficiency: 1.0,
            attributes: HashMap::new(),
            specializes: None,
            availability: None,
        }
    }

    /// Set the resource name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set the capacity
    pub fn capacity(mut self, capacity: f32) -> Self {
        self.capacity = capacity;
        self
    }

    /// Set the cost rate
    pub fn rate(mut self, rate: Money) -> Self {
        self.rate = Some(rate);
        self
    }

    /// Set the efficiency factor
    pub fn efficiency(mut self, efficiency: f32) -> Self {
        self.efficiency = efficiency;
        self
    }

    /// Set the profile this resource specializes (RFC-0001)
    pub fn specializes(mut self, profile: impl Into<String>) -> Self {
        self.specializes = Some(profile.into());
        self
    }

    /// Set the availability (RFC-0001)
    ///
    /// Availability is multiplied with calendar hours to get effective capacity.
    /// Example: 0.8 availability × 8h/day calendar = 6.4 effective hours/day
    pub fn availability(mut self, availability: f32) -> Self {
        self.availability = Some(availability);
        self
    }

    /// Get effective availability (defaults to 1.0 if not set)
    pub fn effective_availability(&self) -> f32 {
        self.availability.unwrap_or(1.0)
    }

    /// Check if this resource specializes a profile
    pub fn is_specialized(&self) -> bool {
        self.specializes.is_some()
    }
}

// ============================================================================
// Calendar
// ============================================================================

/// Working time definitions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Calendar {
    /// Unique identifier
    pub id: CalendarId,
    /// Human-readable name
    pub name: String,
    /// Working hours per day
    pub working_hours: Vec<TimeRange>,
    /// Working days (0 = Sunday, 6 = Saturday)
    pub working_days: Vec<u8>,
    /// Holiday dates
    pub holidays: Vec<Holiday>,
    /// Exceptions (override working hours for specific dates)
    pub exceptions: Vec<CalendarException>,
}

impl Default for Calendar {
    fn default() -> Self {
        Self {
            id: "default".into(),
            name: "Standard".into(),
            working_hours: vec![
                TimeRange { start: 9 * 60, end: 12 * 60 },
                TimeRange { start: 13 * 60, end: 17 * 60 },
            ],
            working_days: vec![1, 2, 3, 4, 5], // Mon-Fri
            holidays: Vec::new(),
            exceptions: Vec::new(),
        }
    }
}

impl Calendar {
    /// Calculate working hours per day
    pub fn hours_per_day(&self) -> f64 {
        self.working_hours.iter().map(|r| r.duration_hours()).sum()
    }

    /// Check if a date is a working day
    pub fn is_working_day(&self, date: NaiveDate) -> bool {
        let weekday = date.weekday().num_days_from_sunday() as u8;
        if !self.working_days.contains(&weekday) {
            return false;
        }
        if self.holidays.iter().any(|h| h.contains(date)) {
            return false;
        }
        true
    }
}

/// Time range within a day (in minutes from midnight)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: u16, // Minutes from midnight
    pub end: u16,
}

impl TimeRange {
    pub fn duration_hours(&self) -> f64 {
        (self.end - self.start) as f64 / 60.0
    }
}

/// Holiday definition
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Holiday {
    pub name: String,
    pub start: NaiveDate,
    pub end: NaiveDate,
}

impl Holiday {
    pub fn contains(&self, date: NaiveDate) -> bool {
        date >= self.start && date <= self.end
    }
}

/// Calendar exception (override for specific dates)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CalendarException {
    pub date: NaiveDate,
    pub working_hours: Option<Vec<TimeRange>>, // None = non-working
}

// ============================================================================
// Scenario
// ============================================================================

/// Alternative scenario for what-if analysis
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scenario {
    pub id: String,
    pub name: String,
    pub parent: Option<String>,
    pub overrides: Vec<ScenarioOverride>,
}

/// Override for a scenario
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ScenarioOverride {
    TaskEffort { task_id: TaskId, effort: Duration },
    TaskDuration { task_id: TaskId, duration: Duration },
    ResourceCapacity { resource_id: ResourceId, capacity: f32 },
}

// ============================================================================
// Schedule (Result)
// ============================================================================

/// The result of scheduling a project
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Schedule {
    /// Scheduled tasks indexed by ID
    pub tasks: HashMap<TaskId, ScheduledTask>,
    /// Tasks on the critical path
    pub critical_path: Vec<TaskId>,
    /// Total project duration
    pub project_duration: Duration,
    /// Project end date
    pub project_end: NaiveDate,
    /// Total project cost (concrete resources only)
    pub total_cost: Option<Money>,
    /// RFC-0001: Total cost range (includes abstract profile assignments)
    pub total_cost_range: Option<CostRange>,

    // Project Status Fields (I004)
    /// Overall project progress (0-100), weighted by task duration
    pub project_progress: u8,
    /// Project baseline finish date (max of all baseline_finish)
    pub project_baseline_finish: NaiveDate,
    /// Project forecast finish date (max of all forecast_finish)
    pub project_forecast_finish: NaiveDate,
    /// Project-level variance in calendar days (forecast - baseline)
    pub project_variance_days: i64,

    // Earned Value Fields (I005)
    /// Planned Value at status date (0-100), weighted % of baseline work due
    pub planned_value: u8,
    /// Earned Value (same as project_progress, 0-100)
    pub earned_value: u8,
    /// Schedule Performance Index (EV / PV), capped at 2.0
    pub spi: f64,
}

/// A task with computed schedule information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScheduledTask {
    /// Task ID
    pub task_id: TaskId,
    /// Scheduled start date
    pub start: NaiveDate,
    /// Scheduled finish date
    pub finish: NaiveDate,
    /// Actual duration
    pub duration: Duration,
    /// Resource assignments with time periods
    pub assignments: Vec<Assignment>,
    /// Slack/float time
    pub slack: Duration,
    /// Is this task on the critical path?
    pub is_critical: bool,
    /// Early start date
    pub early_start: NaiveDate,
    /// Early finish date
    pub early_finish: NaiveDate,
    /// Late start date
    pub late_start: NaiveDate,
    /// Late finish date
    pub late_finish: NaiveDate,

    // ========================================================================
    // Progress Tracking Fields
    // ========================================================================

    /// Forecast start (actual_start if available, otherwise planned start)
    pub forecast_start: NaiveDate,
    /// Forecast finish date (calculated based on progress)
    pub forecast_finish: NaiveDate,
    /// Remaining duration based on progress
    pub remaining_duration: Duration,
    /// Completion percentage (0-100)
    pub percent_complete: u8,
    /// Current task status
    pub status: TaskStatus,

    // ========================================================================
    // Variance Fields (Baseline vs Forecast)
    // ========================================================================

    /// Baseline start (planned start ignoring progress)
    pub baseline_start: NaiveDate,
    /// Baseline finish (planned finish ignoring progress)
    pub baseline_finish: NaiveDate,
    /// Start variance in days (forecast_start - baseline_start, positive = late)
    pub start_variance_days: i64,
    /// Finish variance in days (forecast_finish - baseline_finish, positive = late)
    pub finish_variance_days: i64,

    // ========================================================================
    // RFC-0001: Cost Range Fields
    // ========================================================================

    /// Task cost range (aggregated from all assignments)
    pub cost_range: Option<CostRange>,
    /// Whether this task has any abstract (profile) assignments
    pub has_abstract_assignments: bool,
}

impl ScheduledTask {
    /// Create a test ScheduledTask with default progress tracking fields.
    /// Useful for unit tests that don't need progress data.
    #[cfg(test)]
    pub fn test_new(
        task_id: impl Into<String>,
        start: NaiveDate,
        finish: NaiveDate,
        duration: Duration,
        slack: Duration,
        is_critical: bool,
    ) -> Self {
        let task_id = task_id.into();
        Self {
            task_id,
            start,
            finish,
            duration,
            assignments: Vec::new(),
            slack,
            is_critical,
            early_start: start,
            early_finish: finish,
            late_start: start,
            late_finish: finish,
            forecast_start: start,
            forecast_finish: finish,
            remaining_duration: duration,
            percent_complete: 0,
            status: TaskStatus::NotStarted,
            baseline_start: start,
            baseline_finish: finish,
            start_variance_days: 0,
            finish_variance_days: 0,
            cost_range: None,
            has_abstract_assignments: false,
        }
    }
}

/// Resource assignment for a specific period
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Assignment {
    pub resource_id: ResourceId,
    pub start: NaiveDate,
    pub finish: NaiveDate,
    pub units: f32,
    pub cost: Option<Money>,
    /// RFC-0001: Cost range for abstract profile assignments
    pub cost_range: Option<CostRange>,
    /// RFC-0001: Whether this is an abstract (profile) assignment
    pub is_abstract: bool,
}

// ============================================================================
// Traits
// ============================================================================

/// Core scheduling abstraction
pub trait Scheduler: Send + Sync {
    /// Compute a schedule for the given project
    fn schedule(&self, project: &Project) -> Result<Schedule, ScheduleError>;

    /// Check if a schedule is feasible without computing it
    fn is_feasible(&self, project: &Project) -> FeasibilityResult;

    /// Explain why a particular scheduling decision was made
    fn explain(&self, project: &Project, task: &TaskId) -> Explanation;
}

/// What-if analysis capabilities (typically BDD-powered)
pub trait WhatIfAnalysis {
    /// Analyze impact of a constraint change
    fn what_if(&self, project: &Project, change: &Constraint) -> WhatIfReport;

    /// Count valid schedules under current constraints
    fn count_solutions(&self, project: &Project) -> num_bigint::BigUint;

    /// Find all critical constraints
    fn critical_constraints(&self, project: &Project) -> Vec<Constraint>;
}

/// Output rendering
pub trait Renderer {
    type Output;

    /// Render a schedule to the output format
    fn render(&self, project: &Project, schedule: &Schedule) -> Result<Self::Output, RenderError>;
}

// ============================================================================
// Result Types
// ============================================================================

/// Result of feasibility check
#[derive(Clone, Debug)]
pub struct FeasibilityResult {
    pub feasible: bool,
    pub conflicts: Vec<Conflict>,
    pub suggestions: Vec<Suggestion>,
}

/// Scheduling conflict
#[derive(Clone, Debug)]
pub struct Conflict {
    pub conflict_type: ConflictType,
    pub description: String,
    pub involved_tasks: Vec<TaskId>,
    pub involved_resources: Vec<ResourceId>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ConflictType {
    CircularDependency,
    ResourceOverallocation,
    ImpossibleConstraint,
    DeadlineMissed,
}

/// Suggestion for resolving issues
#[derive(Clone, Debug)]
pub struct Suggestion {
    pub description: String,
    pub impact: String,
}

/// Type of effect a constraint has on scheduling
#[derive(Clone, Debug, PartialEq)]
pub enum ConstraintEffectType {
    /// Constraint pushed ES/EF forward (floor constraint active)
    PushedStart,
    /// Constraint capped LS/LF (ceiling constraint active)
    CappedLate,
    /// Constraint pinned task to specific date (MustStartOn/MustFinishOn)
    Pinned,
    /// Constraint was redundant (dependencies already more restrictive)
    Redundant,
}

/// Effect of a temporal constraint on task scheduling
#[derive(Clone, Debug)]
pub struct ConstraintEffect {
    /// The constraint that was applied
    pub constraint: TaskConstraint,
    /// What effect the constraint had
    pub effect: ConstraintEffectType,
    /// Human-readable description of the effect
    pub description: String,
}

/// Explanation of a scheduling decision
#[derive(Clone, Debug)]
pub struct Explanation {
    pub task_id: TaskId,
    pub reason: String,
    pub constraints_applied: Vec<String>,
    pub alternatives_considered: Vec<String>,
    /// Detailed effects of temporal constraints (Phase 4)
    pub constraint_effects: Vec<ConstraintEffect>,
}

/// Constraint for what-if analysis
#[derive(Clone, Debug)]
pub enum Constraint {
    TaskEffort { task_id: TaskId, effort: Duration },
    TaskDuration { task_id: TaskId, duration: Duration },
    ResourceCapacity { resource_id: ResourceId, capacity: f32 },
    Deadline { date: NaiveDate },
}

/// Result of what-if analysis
#[derive(Clone, Debug)]
pub struct WhatIfReport {
    pub still_feasible: bool,
    pub solutions_before: num_bigint::BigUint,
    pub solutions_after: num_bigint::BigUint,
    pub newly_critical: Vec<TaskId>,
    pub schedule_delta: Option<Duration>,
    pub cost_delta: Option<Money>,
}

// ============================================================================
// Errors
// ============================================================================

/// Scheduling error
#[derive(Debug, Error)]
pub enum ScheduleError {
    #[error("Circular dependency detected: {0}")]
    CircularDependency(String),

    #[error("Resource not found: {0}")]
    ResourceNotFound(ResourceId),

    #[error("Task not found: {0}")]
    TaskNotFound(TaskId),

    #[error("Calendar not found: {0}")]
    CalendarNotFound(CalendarId),

    #[error("Infeasible schedule: {0}")]
    Infeasible(String),

    #[error("Constraint violation: {0}")]
    ConstraintViolation(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Rendering error
#[derive(Debug, Error)]
pub enum RenderError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Format error: {0}")]
    Format(String),

    #[error("Invalid data: {0}")]
    InvalidData(String),
}

// ============================================================================
// Diagnostics
// ============================================================================

/// Diagnostic severity level
///
/// Severity determines how the diagnostic is treated by the CLI:
/// - Error: Always fatal, blocks completion
/// - Warning: Likely problem, becomes error in --strict mode
/// - Hint: Suggestion, becomes warning in --strict mode
/// - Info: Informational, unchanged in --strict mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Error,
    Warning,
    Hint,
    Info,
}

impl Severity {
    /// Returns the string prefix used in diagnostic output (e.g., "error", "warning")
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Hint => "hint",
            Severity::Info => "info",
        }
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Diagnostic code identifying the specific diagnostic type
///
/// Codes are stable identifiers used for:
/// - Machine-readable output (JSON)
/// - Documentation references
/// - Suppression/filtering
///
/// Naming convention: {Severity prefix}{Number}{Description}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticCode {
    // Errors (E) - Cannot proceed
    /// Circular specialization in profile inheritance chain
    E001CircularSpecialization,
    /// Profile has no rate and is used in cost-bearing assignments
    E002ProfileWithoutRate,
    /// Task constraint cannot be satisfied (ES > LS)
    E003InfeasibleConstraint,

    // Calendar Errors (C001-C009)
    /// Calendar has no working hours defined
    C001ZeroWorkingHours,
    /// Calendar has no working days defined
    C002NoWorkingDays,

    // Warnings (W) - Likely problem
    /// Task assigned to abstract profile instead of concrete resource
    W001AbstractAssignment,
    /// Task cost range spread exceeds threshold
    W002WideCostRange,
    /// Profile references undefined trait
    W003UnknownTrait,
    /// Resource leveling could not fully resolve all conflicts
    W004ApproximateLeveling,
    /// Task constraint reduces slack to zero (now on critical path)
    W005ConstraintZeroSlack,
    /// Task is slipping beyond threshold (forecast > baseline)
    W006ScheduleVariance,
    /// Container has dependencies but child task has none (MS Project compatibility)
    W014ContainerDependency,

    // Calendar Warnings (C010-C019)
    /// Task scheduled on non-working day
    C010NonWorkingDay,
    /// Task and assigned resource use different calendars
    C011CalendarMismatch,

    // Hints (H) - Suggestions
    /// Task has both concrete and abstract assignments
    H001MixedAbstraction,
    /// Profile is defined but never assigned
    H002UnusedProfile,
    /// Trait is defined but never referenced
    H003UnusedTrait,
    /// Task has no predecessors or date constraints (dangling/orphan task)
    H004TaskUnconstrained,

    // Calendar Hints (C020-C029)
    /// Calendar has low availability (< 40% working days)
    C020LowAvailability,
    /// Calendar is missing common holiday
    C021MissingCommonHoliday,
    /// Calendar has suspicious working hours (e.g., 24/7)
    C022SuspiciousHours,
    /// Holiday falls on non-working day (redundant)
    C023RedundantHoliday,

    // Info (I) - Informational
    /// Project scheduling summary
    I001ProjectCostSummary,
    /// Refinement progress status
    I002RefinementProgress,
    /// Resource utilization summary
    I003ResourceUtilization,
    /// Project status summary (overall progress and variance)
    I004ProjectStatus,
    /// Earned value summary (EV, PV, SPI)
    I005EarnedValueSummary,
}

impl DiagnosticCode {
    /// Returns the short code string (e.g., "E001", "W002")
    pub fn as_str(&self) -> &'static str {
        match self {
            DiagnosticCode::E001CircularSpecialization => "E001",
            DiagnosticCode::E002ProfileWithoutRate => "E002",
            DiagnosticCode::E003InfeasibleConstraint => "E003",
            DiagnosticCode::C001ZeroWorkingHours => "C001",
            DiagnosticCode::C002NoWorkingDays => "C002",
            DiagnosticCode::W001AbstractAssignment => "W001",
            DiagnosticCode::W002WideCostRange => "W002",
            DiagnosticCode::W003UnknownTrait => "W003",
            DiagnosticCode::W004ApproximateLeveling => "W004",
            DiagnosticCode::W005ConstraintZeroSlack => "W005",
            DiagnosticCode::W006ScheduleVariance => "W006",
            DiagnosticCode::W014ContainerDependency => "W014",
            DiagnosticCode::C010NonWorkingDay => "C010",
            DiagnosticCode::C011CalendarMismatch => "C011",
            DiagnosticCode::H001MixedAbstraction => "H001",
            DiagnosticCode::H002UnusedProfile => "H002",
            DiagnosticCode::H003UnusedTrait => "H003",
            DiagnosticCode::H004TaskUnconstrained => "H004",
            DiagnosticCode::C020LowAvailability => "C020",
            DiagnosticCode::C021MissingCommonHoliday => "C021",
            DiagnosticCode::C022SuspiciousHours => "C022",
            DiagnosticCode::C023RedundantHoliday => "C023",
            DiagnosticCode::I001ProjectCostSummary => "I001",
            DiagnosticCode::I002RefinementProgress => "I002",
            DiagnosticCode::I003ResourceUtilization => "I003",
            DiagnosticCode::I004ProjectStatus => "I004",
            DiagnosticCode::I005EarnedValueSummary => "I005",
        }
    }

    /// Returns the default severity for this diagnostic code
    pub fn default_severity(&self) -> Severity {
        match self {
            DiagnosticCode::E001CircularSpecialization => Severity::Error,
            DiagnosticCode::E002ProfileWithoutRate => Severity::Warning, // Error in strict mode
            DiagnosticCode::E003InfeasibleConstraint => Severity::Error,
            DiagnosticCode::C001ZeroWorkingHours => Severity::Error,
            DiagnosticCode::C002NoWorkingDays => Severity::Error,
            DiagnosticCode::W001AbstractAssignment => Severity::Warning,
            DiagnosticCode::W002WideCostRange => Severity::Warning,
            DiagnosticCode::W003UnknownTrait => Severity::Warning,
            DiagnosticCode::W004ApproximateLeveling => Severity::Warning,
            DiagnosticCode::W005ConstraintZeroSlack => Severity::Warning,
            DiagnosticCode::W006ScheduleVariance => Severity::Warning,
            DiagnosticCode::W014ContainerDependency => Severity::Warning,
            DiagnosticCode::C010NonWorkingDay => Severity::Warning,
            DiagnosticCode::C011CalendarMismatch => Severity::Warning,
            DiagnosticCode::H001MixedAbstraction => Severity::Hint,
            DiagnosticCode::H002UnusedProfile => Severity::Hint,
            DiagnosticCode::H003UnusedTrait => Severity::Hint,
            DiagnosticCode::H004TaskUnconstrained => Severity::Hint,
            DiagnosticCode::C020LowAvailability => Severity::Hint,
            DiagnosticCode::C021MissingCommonHoliday => Severity::Hint,
            DiagnosticCode::C022SuspiciousHours => Severity::Hint,
            DiagnosticCode::C023RedundantHoliday => Severity::Hint,
            DiagnosticCode::I001ProjectCostSummary => Severity::Info,
            DiagnosticCode::I002RefinementProgress => Severity::Info,
            DiagnosticCode::I003ResourceUtilization => Severity::Info,
            DiagnosticCode::I004ProjectStatus => Severity::Info,
            DiagnosticCode::I005EarnedValueSummary => Severity::Info,
        }
    }

    /// Returns the diagnostic ordering priority (lower = emitted first)
    ///
    /// Ordering: Errors → Cost warnings → Assignment warnings → Hints → Info
    pub fn ordering_priority(&self) -> u8 {
        match self {
            // Structural errors first
            DiagnosticCode::E001CircularSpecialization => 0,
            DiagnosticCode::E002ProfileWithoutRate => 1,
            DiagnosticCode::E003InfeasibleConstraint => 2,
            // Calendar errors
            DiagnosticCode::C001ZeroWorkingHours => 3,
            DiagnosticCode::C002NoWorkingDays => 4,
            // Cost-related warnings
            DiagnosticCode::W002WideCostRange => 10,
            DiagnosticCode::W004ApproximateLeveling => 11,
            // Constraint warnings
            DiagnosticCode::W005ConstraintZeroSlack => 12,
            // Schedule variance warnings
            DiagnosticCode::W006ScheduleVariance => 13,
            // MS Project compatibility warnings
            DiagnosticCode::W014ContainerDependency => 14,
            // Calendar warnings
            DiagnosticCode::C010NonWorkingDay => 15,
            DiagnosticCode::C011CalendarMismatch => 16,
            // Assignment-related warnings
            DiagnosticCode::W001AbstractAssignment => 20,
            DiagnosticCode::W003UnknownTrait => 21,
            // Hints
            DiagnosticCode::H001MixedAbstraction => 30,
            DiagnosticCode::H002UnusedProfile => 31,
            DiagnosticCode::H003UnusedTrait => 32,
            DiagnosticCode::H004TaskUnconstrained => 33,
            // Calendar hints
            DiagnosticCode::C020LowAvailability => 34,
            DiagnosticCode::C021MissingCommonHoliday => 35,
            DiagnosticCode::C022SuspiciousHours => 36,
            DiagnosticCode::C023RedundantHoliday => 37,
            // Info last
            DiagnosticCode::I001ProjectCostSummary => 40,
            DiagnosticCode::I002RefinementProgress => 41,
            DiagnosticCode::I003ResourceUtilization => 42,
            DiagnosticCode::I004ProjectStatus => 43,
            DiagnosticCode::I005EarnedValueSummary => 44,
        }
    }
}

impl std::fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Source location span for diagnostic highlighting
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSpan {
    /// Line number (1-based)
    pub line: usize,
    /// Column number (1-based)
    pub column: usize,
    /// Length of the span in characters
    pub length: usize,
    /// Optional label for this span
    pub label: Option<String>,
}

impl SourceSpan {
    pub fn new(line: usize, column: usize, length: usize) -> Self {
        Self {
            line,
            column,
            length,
            label: None,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// A diagnostic message emitted during analysis or scheduling
///
/// Diagnostics are structured data representing errors, warnings, hints,
/// or informational messages. The message text is fully rendered at
/// emission time according to the specification in DIAGNOSTICS.md.
///
/// # Example
///
/// ```
/// use utf8proj_core::{Diagnostic, DiagnosticCode, Severity};
///
/// let diagnostic = Diagnostic::new(
///     DiagnosticCode::W001AbstractAssignment,
///     "task 'api_dev' is assigned to abstract profile 'developer'"
/// );
///
/// assert_eq!(diagnostic.severity, Severity::Warning);
/// assert_eq!(diagnostic.code.as_str(), "W001");
/// ```
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// The diagnostic code
    pub code: DiagnosticCode,
    /// Severity level (derived from code by default)
    pub severity: Severity,
    /// The primary message (fully rendered)
    pub message: String,
    /// Source file path (if applicable)
    pub file: Option<std::path::PathBuf>,
    /// Primary source span (if applicable)
    pub span: Option<SourceSpan>,
    /// Additional spans for related locations
    pub secondary_spans: Vec<SourceSpan>,
    /// Additional notes (displayed after the main message)
    pub notes: Vec<String>,
    /// Hints for fixing the issue
    pub hints: Vec<String>,
}

impl Diagnostic {
    /// Create a new diagnostic with the given code and message
    ///
    /// Severity is derived from the diagnostic code's default.
    pub fn new(code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            severity: code.default_severity(),
            code,
            message: message.into(),
            file: None,
            span: None,
            secondary_spans: Vec::new(),
            notes: Vec::new(),
            hints: Vec::new(),
        }
    }

    /// Create an error diagnostic
    pub fn error(code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            code,
            message: message.into(),
            file: None,
            span: None,
            secondary_spans: Vec::new(),
            notes: Vec::new(),
            hints: Vec::new(),
        }
    }

    /// Create a warning diagnostic
    pub fn warning(code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            code,
            message: message.into(),
            file: None,
            span: None,
            secondary_spans: Vec::new(),
            notes: Vec::new(),
            hints: Vec::new(),
        }
    }

    /// Set the source file
    pub fn with_file(mut self, file: impl Into<std::path::PathBuf>) -> Self {
        self.file = Some(file.into());
        self
    }

    /// Set the primary source span
    pub fn with_span(mut self, span: SourceSpan) -> Self {
        self.span = Some(span);
        self
    }

    /// Add a secondary span
    pub fn with_secondary_span(mut self, span: SourceSpan) -> Self {
        self.secondary_spans.push(span);
        self
    }

    /// Add a note
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    /// Add a hint
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hints.push(hint.into());
        self
    }

    /// Returns true if this is an error-level diagnostic
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }

    /// Returns true if this is a warning-level diagnostic
    pub fn is_warning(&self) -> bool {
        self.severity == Severity::Warning
    }
}

/// Trait for receiving diagnostic messages
///
/// Implementations handle formatting and output of diagnostics.
/// The solver and analyzer emit diagnostics through this trait,
/// allowing different backends (CLI, LSP, tests) to handle them.
///
/// # Example
///
/// ```
/// use utf8proj_core::{Diagnostic, DiagnosticEmitter};
///
/// struct CollectingEmitter {
///     diagnostics: Vec<Diagnostic>,
/// }
///
/// impl DiagnosticEmitter for CollectingEmitter {
///     fn emit(&mut self, diagnostic: Diagnostic) {
///         self.diagnostics.push(diagnostic);
///     }
/// }
/// ```
pub trait DiagnosticEmitter {
    /// Emit a diagnostic
    fn emit(&mut self, diagnostic: Diagnostic);
}

/// A simple diagnostic emitter that collects diagnostics into a Vec
///
/// Useful for testing and batch processing.
#[derive(Debug, Default)]
pub struct CollectingEmitter {
    pub diagnostics: Vec<Diagnostic>,
}

impl CollectingEmitter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if any errors were emitted
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.is_error())
    }

    /// Returns the count of errors
    pub fn error_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.is_error()).count()
    }

    /// Returns the count of warnings
    pub fn warning_count(&self) -> usize {
        self.diagnostics.iter().filter(|d| d.is_warning()).count()
    }

    /// Returns diagnostics sorted by ordering priority, then by source location
    pub fn sorted(&self) -> Vec<&Diagnostic> {
        let mut sorted: Vec<_> = self.diagnostics.iter().collect();
        sorted.sort_by(|a, b| {
            // First by ordering priority
            let priority_cmp = a.code.ordering_priority().cmp(&b.code.ordering_priority());
            if priority_cmp != std::cmp::Ordering::Equal {
                return priority_cmp;
            }
            // Then by file
            let file_cmp = a.file.cmp(&b.file);
            if file_cmp != std::cmp::Ordering::Equal {
                return file_cmp;
            }
            // Then by line
            let line_a = a.span.as_ref().map(|s| s.line).unwrap_or(0);
            let line_b = b.span.as_ref().map(|s| s.line).unwrap_or(0);
            let line_cmp = line_a.cmp(&line_b);
            if line_cmp != std::cmp::Ordering::Equal {
                return line_cmp;
            }
            // Then by column
            let col_a = a.span.as_ref().map(|s| s.column).unwrap_or(0);
            let col_b = b.span.as_ref().map(|s| s.column).unwrap_or(0);
            col_a.cmp(&col_b)
        });
        sorted
    }
}

impl DiagnosticEmitter for CollectingEmitter {
    fn emit(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_arithmetic() {
        let d1 = Duration::days(5);
        let d2 = Duration::days(3);
        assert_eq!((d1 + d2).as_days(), 8.0);
        assert_eq!((d1 - d2).as_days(), 2.0);
    }

    #[test]
    fn task_builder() {
        let task = Task::new("impl")
            .name("Implementation")
            .effort(Duration::days(10))
            .depends_on("design")
            .assign("dev")
            .priority(700);

        assert_eq!(task.id, "impl");
        assert_eq!(task.name, "Implementation");
        assert_eq!(task.effort, Some(Duration::days(10)));
        assert_eq!(task.depends.len(), 1);
        assert_eq!(task.assigned.len(), 1);
        assert_eq!(task.priority, 700);
    }

    #[test]
    fn calendar_working_day() {
        let cal = Calendar::default();
        
        // Monday
        let monday = NaiveDate::from_ymd_opt(2025, 2, 3).unwrap();
        assert!(cal.is_working_day(monday));
        
        // Saturday
        let saturday = NaiveDate::from_ymd_opt(2025, 2, 1).unwrap();
        assert!(!cal.is_working_day(saturday));
    }

    #[test]
    fn project_leaf_tasks() {
        let project = Project {
            id: "test".into(),
            name: "Test".into(),
            start: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            end: None,
            calendar: "default".into(),
            currency: "USD".into(),
            tasks: vec![
                Task::new("parent")
                    .child(Task::new("child1"))
                    .child(Task::new("child2")),
                Task::new("standalone"),
            ],
            resources: Vec::new(),
            calendars: vec![Calendar::default()],
            scenarios: Vec::new(),
            attributes: HashMap::new(),
            profiles: Vec::new(),
            traits: Vec::new(),
            cost_policy: CostPolicy::default(),
        };

        let leaves = project.leaf_tasks();
        assert_eq!(leaves.len(), 3);
        assert!(leaves.iter().any(|t| t.id == "child1"));
        assert!(leaves.iter().any(|t| t.id == "child2"));
        assert!(leaves.iter().any(|t| t.id == "standalone"));
    }

    #[test]
    fn duration_constructors() {
        // Test minutes constructor
        let d_min = Duration::minutes(120);
        assert_eq!(d_min.minutes, 120);
        assert_eq!(d_min.as_hours(), 2.0);

        // Test hours constructor
        let d_hours = Duration::hours(3);
        assert_eq!(d_hours.minutes, 180);
        assert_eq!(d_hours.as_hours(), 3.0);

        // Test weeks constructor (5 days * 8 hours)
        let d_weeks = Duration::weeks(1);
        assert_eq!(d_weeks.minutes, 5 * 8 * 60);
        assert_eq!(d_weeks.as_days(), 5.0);
    }

    #[test]
    fn project_get_task_nested() {
        let project = Project {
            id: "test".into(),
            name: "Test".into(),
            start: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            end: None,
            calendar: "default".into(),
            currency: "USD".into(),
            tasks: vec![
                Task::new("parent")
                    .name("Parent Task")
                    .child(Task::new("child1").name("Child 1"))
                    .child(Task::new("child2")
                        .name("Child 2")
                        .child(Task::new("grandchild").name("Grandchild"))),
                Task::new("standalone").name("Standalone"),
            ],
            resources: Vec::new(),
            calendars: vec![Calendar::default()],
            scenarios: Vec::new(),
            attributes: HashMap::new(),
            profiles: Vec::new(),
            traits: Vec::new(),
            cost_policy: CostPolicy::default(),
        };

        // Find top-level task
        let standalone = project.get_task("standalone");
        assert!(standalone.is_some());
        assert_eq!(standalone.unwrap().name, "Standalone");

        // Find nested task (depth 1)
        let child1 = project.get_task("child1");
        assert!(child1.is_some());
        assert_eq!(child1.unwrap().name, "Child 1");

        // Find deeply nested task (depth 2)
        let grandchild = project.get_task("grandchild");
        assert!(grandchild.is_some());
        assert_eq!(grandchild.unwrap().name, "Grandchild");

        // Non-existent task
        let missing = project.get_task("nonexistent");
        assert!(missing.is_none());
    }

    #[test]
    fn project_get_resource() {
        let project = Project {
            id: "test".into(),
            name: "Test".into(),
            start: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            end: None,
            calendar: "default".into(),
            currency: "USD".into(),
            tasks: Vec::new(),
            resources: vec![
                Resource::new("dev1").name("Developer 1"),
                Resource::new("pm").name("Project Manager"),
            ],
            calendars: vec![Calendar::default()],
            scenarios: Vec::new(),
            attributes: HashMap::new(),
            profiles: Vec::new(),
            traits: Vec::new(),
            cost_policy: CostPolicy::default(),
        };

        let dev = project.get_resource("dev1");
        assert!(dev.is_some());
        assert_eq!(dev.unwrap().name, "Developer 1");

        let pm = project.get_resource("pm");
        assert!(pm.is_some());
        assert_eq!(pm.unwrap().name, "Project Manager");

        let missing = project.get_resource("nonexistent");
        assert!(missing.is_none());
    }

    #[test]
    fn task_is_summary() {
        let leaf_task = Task::new("leaf").name("Leaf Task");
        assert!(!leaf_task.is_summary());

        let summary_task = Task::new("summary")
            .name("Summary Task")
            .child(Task::new("child1"))
            .child(Task::new("child2"));
        assert!(summary_task.is_summary());
    }

    #[test]
    fn task_assign_with_units() {
        let task = Task::new("task1")
            .assign("dev1")
            .assign_with_units("dev2", 0.5)
            .assign_with_units("contractor", 0.25);

        assert_eq!(task.assigned.len(), 3);
        assert_eq!(task.assigned[0].units, 1.0); // Default assignment
        assert_eq!(task.assigned[1].units, 0.5); // Partial assignment
        assert_eq!(task.assigned[2].units, 0.25); // Quarter assignment
    }

    #[test]
    fn resource_efficiency() {
        let resource = Resource::new("dev")
            .name("Developer")
            .efficiency(0.8);

        assert_eq!(resource.efficiency, 0.8);
    }

    #[test]
    fn calendar_hours_per_day() {
        let cal = Calendar::default();
        // Default: 9:00-12:00 (3h) + 13:00-17:00 (4h) = 7 hours
        assert_eq!(cal.hours_per_day(), 7.0);
    }

    #[test]
    fn time_range_duration() {
        let range = TimeRange {
            start: 9 * 60,  // 9:00 AM
            end: 17 * 60,   // 5:00 PM
        };
        assert_eq!(range.duration_hours(), 8.0);

        let half_day = TimeRange {
            start: 9 * 60,
            end: 13 * 60,
        };
        assert_eq!(half_day.duration_hours(), 4.0);
    }

    #[test]
    fn holiday_contains_date() {
        let holiday = Holiday {
            name: "Winter Break".into(),
            start: NaiveDate::from_ymd_opt(2025, 12, 24).unwrap(),
            end: NaiveDate::from_ymd_opt(2025, 12, 26).unwrap(),
        };

        // Before holiday
        assert!(!holiday.contains(NaiveDate::from_ymd_opt(2025, 12, 23).unwrap()));

        // First day of holiday
        assert!(holiday.contains(NaiveDate::from_ymd_opt(2025, 12, 24).unwrap()));

        // Middle of holiday
        assert!(holiday.contains(NaiveDate::from_ymd_opt(2025, 12, 25).unwrap()));

        // Last day of holiday
        assert!(holiday.contains(NaiveDate::from_ymd_opt(2025, 12, 26).unwrap()));

        // After holiday
        assert!(!holiday.contains(NaiveDate::from_ymd_opt(2025, 12, 27).unwrap()));
    }

    #[test]
    fn task_milestone() {
        let milestone = Task::new("ms1")
            .name("Phase Complete")
            .milestone();

        assert!(milestone.milestone);
        assert_eq!(milestone.duration, Some(Duration::zero()));
    }

    #[test]
    fn depends_on_creates_fs_dependency() {
        let task = Task::new("task").depends_on("pred");

        assert_eq!(task.depends.len(), 1);
        let dep = &task.depends[0];
        assert_eq!(dep.predecessor, "pred");
        assert_eq!(dep.dep_type, DependencyType::FinishToStart);
        assert!(dep.lag.is_none());
    }

    #[test]
    fn with_dependency_preserves_all_fields() {
        let dep = Dependency {
            predecessor: "other".into(),
            dep_type: DependencyType::StartToStart,
            lag: Some(Duration::days(2)),
        };
        let task = Task::new("task").with_dependency(dep);

        assert_eq!(task.depends.len(), 1);
        let d = &task.depends[0];
        assert_eq!(d.predecessor, "other");
        assert_eq!(d.dep_type, DependencyType::StartToStart);
        assert_eq!(d.lag, Some(Duration::days(2)));
    }

    #[test]
    fn assign_sets_full_allocation() {
        let task = Task::new("task").assign("dev");

        assert_eq!(task.assigned.len(), 1);
        assert_eq!(task.assigned[0].resource_id, "dev");
        assert_eq!(task.assigned[0].units, 1.0);
    }

    #[test]
    fn assign_with_units_sets_custom_allocation() {
        let task = Task::new("task").assign_with_units("dev", 0.75);

        assert_eq!(task.assigned.len(), 1);
        assert_eq!(task.assigned[0].resource_id, "dev");
        assert_eq!(task.assigned[0].units, 0.75);
    }

    // ========================================================================
    // Progress Tracking Tests
    // ========================================================================

    #[test]
    fn remaining_duration_linear_interpolation() {
        // 10-day task at 60% complete → 4 days remaining
        let task = Task::new("task")
            .duration(Duration::days(10))
            .complete(60.0);

        let remaining = task.remaining_duration();
        assert_eq!(remaining.as_days(), 4.0);
    }

    #[test]
    fn remaining_duration_zero_complete() {
        let task = Task::new("task")
            .duration(Duration::days(10));

        let remaining = task.remaining_duration();
        assert_eq!(remaining.as_days(), 10.0);
    }

    #[test]
    fn remaining_duration_fully_complete() {
        let task = Task::new("task")
            .duration(Duration::days(10))
            .complete(100.0);

        let remaining = task.remaining_duration();
        assert_eq!(remaining.as_days(), 0.0);
    }

    #[test]
    fn remaining_duration_uses_effort_if_no_duration() {
        let task = Task::new("task")
            .effort(Duration::days(20))
            .complete(50.0);

        let remaining = task.remaining_duration();
        assert_eq!(remaining.as_days(), 10.0);
    }

    #[test]
    fn effective_percent_complete_default() {
        let task = Task::new("task");
        assert_eq!(task.effective_percent_complete(), 0);
    }

    #[test]
    fn effective_percent_complete_clamped() {
        // Clamp above 100
        let task = Task::new("task").complete(150.0);
        assert_eq!(task.effective_percent_complete(), 100);

        // Clamp below 0
        let task = Task::new("task").complete(-10.0);
        assert_eq!(task.effective_percent_complete(), 0);
    }

    #[test]
    fn derived_status_not_started() {
        let task = Task::new("task");
        assert_eq!(task.derived_status(), TaskStatus::NotStarted);
    }

    #[test]
    fn derived_status_in_progress_from_percent() {
        let task = Task::new("task").complete(50.0);
        assert_eq!(task.derived_status(), TaskStatus::InProgress);
    }

    #[test]
    fn derived_status_in_progress_from_actual_start() {
        let task = Task::new("task")
            .actual_start(NaiveDate::from_ymd_opt(2026, 1, 15).unwrap());
        assert_eq!(task.derived_status(), TaskStatus::InProgress);
    }

    #[test]
    fn derived_status_complete_from_percent() {
        let task = Task::new("task").complete(100.0);
        assert_eq!(task.derived_status(), TaskStatus::Complete);
    }

    #[test]
    fn derived_status_complete_from_actual_finish() {
        let task = Task::new("task")
            .actual_finish(NaiveDate::from_ymd_opt(2026, 1, 20).unwrap());
        assert_eq!(task.derived_status(), TaskStatus::Complete);
    }

    #[test]
    fn derived_status_explicit_overrides() {
        // Even with 100% complete, explicit status takes precedence
        let task = Task::new("task")
            .complete(100.0)
            .with_status(TaskStatus::Blocked);
        assert_eq!(task.derived_status(), TaskStatus::Blocked);
    }

    #[test]
    fn task_status_display() {
        assert_eq!(format!("{}", TaskStatus::NotStarted), "Not Started");
        assert_eq!(format!("{}", TaskStatus::InProgress), "In Progress");
        assert_eq!(format!("{}", TaskStatus::Complete), "Complete");
        assert_eq!(format!("{}", TaskStatus::Blocked), "Blocked");
        assert_eq!(format!("{}", TaskStatus::AtRisk), "At Risk");
        assert_eq!(format!("{}", TaskStatus::OnHold), "On Hold");
    }

    #[test]
    fn task_builder_with_progress_fields() {
        let date_start = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
        let date_finish = NaiveDate::from_ymd_opt(2026, 1, 20).unwrap();

        let task = Task::new("task")
            .duration(Duration::days(5))
            .complete(75.0)
            .actual_start(date_start)
            .actual_finish(date_finish)
            .with_status(TaskStatus::Complete);

        assert_eq!(task.complete, Some(75.0));
        assert_eq!(task.actual_start, Some(date_start));
        assert_eq!(task.actual_finish, Some(date_finish));
        assert_eq!(task.status, Some(TaskStatus::Complete));
    }

    // ========================================================================
    // Container Progress Tests
    // ========================================================================

    #[test]
    fn container_progress_weighted_average() {
        // Container with 3 children of different durations and progress
        // Backend: 20d @ 60%, Frontend: 15d @ 30%, Testing: 10d @ 0%
        // Expected: (20*60 + 15*30 + 10*0) / (20+15+10) = 1650/45 = 36.67 ≈ 37%
        let container = Task::new("development")
            .child(Task::new("backend").duration(Duration::days(20)).complete(60.0))
            .child(Task::new("frontend").duration(Duration::days(15)).complete(30.0))
            .child(Task::new("testing").duration(Duration::days(10)));

        assert!(container.is_container());
        assert_eq!(container.container_progress(), Some(37));
    }

    #[test]
    fn container_progress_empty_container() {
        let container = Task::new("empty");
        assert!(!container.is_container());
        assert_eq!(container.container_progress(), None);
    }

    #[test]
    fn container_progress_all_complete() {
        let container = Task::new("done")
            .child(Task::new("a").duration(Duration::days(5)).complete(100.0))
            .child(Task::new("b").duration(Duration::days(5)).complete(100.0));

        assert_eq!(container.container_progress(), Some(100));
    }

    #[test]
    fn container_progress_none_started() {
        let container = Task::new("pending")
            .child(Task::new("a").duration(Duration::days(5)))
            .child(Task::new("b").duration(Duration::days(5)));

        assert_eq!(container.container_progress(), Some(0));
    }

    #[test]
    fn container_progress_nested_containers() {
        // Nested structure:
        // project
        // ├── phase1 (container)
        // │   ├── task_a: 10d @ 100%
        // │   └── task_b: 10d @ 50%
        // └── phase2 (container)
        //     └── task_c: 20d @ 25%
        //
        // Phase1: (10*100 + 10*50) / 20 = 75%
        // Total: (10*100 + 10*50 + 20*25) / 40 = 2000/40 = 50%
        let project = Task::new("project")
            .child(
                Task::new("phase1")
                    .child(Task::new("task_a").duration(Duration::days(10)).complete(100.0))
                    .child(Task::new("task_b").duration(Duration::days(10)).complete(50.0)),
            )
            .child(
                Task::new("phase2")
                    .child(Task::new("task_c").duration(Duration::days(20)).complete(25.0)),
            );

        // Check nested container progress
        let phase1 = &project.children[0];
        assert_eq!(phase1.container_progress(), Some(75));

        // Check top-level container progress (flattens all leaves)
        assert_eq!(project.container_progress(), Some(50));
    }

    #[test]
    fn container_progress_effective_with_override() {
        // Container with explicit progress set (manual override)
        let container = Task::new("dev")
            .complete(80.0) // Manual override
            .child(Task::new("a").duration(Duration::days(10)).complete(50.0))
            .child(Task::new("b").duration(Duration::days(10)).complete(50.0));

        // Derived would be 50%, but manual override is 80%
        assert_eq!(container.container_progress(), Some(50));
        assert_eq!(container.effective_progress(), 80); // Uses override
    }

    #[test]
    fn container_progress_mismatch_detection() {
        let container = Task::new("dev")
            .complete(80.0) // Claims 80%
            .child(Task::new("a").duration(Duration::days(10)).complete(30.0))
            .child(Task::new("b").duration(Duration::days(10)).complete(30.0));

        // Derived is 30%, claimed is 80% - 50% mismatch
        let mismatch = container.progress_mismatch(20);
        assert!(mismatch.is_some());
        let (manual, derived) = mismatch.unwrap();
        assert_eq!(manual, 80);
        assert_eq!(derived, 30);

        // No mismatch if threshold is high
        assert!(container.progress_mismatch(60).is_none());
    }

    #[test]
    fn container_progress_uses_effort_fallback() {
        // When duration not set, should use effort
        let container = Task::new("dev")
            .child(Task::new("a").effort(Duration::days(5)).complete(100.0))
            .child(Task::new("b").effort(Duration::days(5)).complete(0.0));

        assert_eq!(container.container_progress(), Some(50));
    }

    #[test]
    fn container_progress_zero_duration_children() {
        // Container with children that have no duration/effort returns None
        let container = Task::new("dev")
            .child(Task::new("a").complete(50.0))  // No duration
            .child(Task::new("b").complete(100.0)); // No duration

        assert_eq!(container.container_progress(), None);
    }

    #[test]
    fn effective_progress_container_no_override() {
        // Container without manual override uses derived progress
        let container = Task::new("dev")
            .child(Task::new("a").duration(Duration::days(10)).complete(100.0))
            .child(Task::new("b").duration(Duration::days(10)).complete(0.0));

        // No complete() set on container, so it derives from children
        assert_eq!(container.effective_progress(), 50);
    }

    #[test]
    fn effective_progress_leaf_no_complete() {
        // Leaf task with no complete set returns 0
        let task = Task::new("leaf").duration(Duration::days(5));
        assert_eq!(task.effective_progress(), 0);
    }

    #[test]
    fn progress_mismatch_leaf_returns_none() {
        // progress_mismatch on a leaf task returns None
        let task = Task::new("leaf").duration(Duration::days(5)).complete(50.0);
        assert!(task.progress_mismatch(10).is_none());
    }

    #[test]
    fn money_new() {
        use rust_decimal::Decimal;
        use std::str::FromStr;
        let money = Money::new(Decimal::from_str("100.50").unwrap(), "EUR");
        assert_eq!(money.amount, Decimal::from_str("100.50").unwrap());
        assert_eq!(money.currency, "EUR");
    }

    #[test]
    fn resource_rate() {
        use rust_decimal::Decimal;
        use std::str::FromStr;
        let resource = Resource::new("dev")
            .name("Developer")
            .rate(Money::new(Decimal::from_str("500").unwrap(), "USD"));

        assert!(resource.rate.is_some());
        assert_eq!(resource.rate.unwrap().amount, Decimal::from_str("500").unwrap());
    }

    #[test]
    fn calendar_with_holiday() {
        let mut cal = Calendar::default();
        cal.holidays.push(Holiday {
            name: "New Year".into(),
            start: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            end: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        });

        // Jan 1 is a Wednesday (working day) but is a holiday
        let jan1 = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        assert!(!cal.is_working_day(jan1));

        // Jan 2 is Thursday, should be working
        let jan2 = NaiveDate::from_ymd_opt(2025, 1, 2).unwrap();
        assert!(cal.is_working_day(jan2));
    }

    #[test]
    fn scheduled_task_test_new() {
        let start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let finish = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        let st = ScheduledTask::test_new("task1", start, finish, Duration::days(5), Duration::zero(), true);

        assert_eq!(st.task_id, "task1");
        assert_eq!(st.start, start);
        assert_eq!(st.finish, finish);
        assert!(st.is_critical);
        assert_eq!(st.assignments.len(), 0);
        assert_eq!(st.percent_complete, 0);
        assert_eq!(st.status, TaskStatus::NotStarted);
    }

    // ========================================================================
    // RFC-0001: Progressive Resource Refinement Tests
    // ========================================================================

    #[test]
    fn trait_builder() {
        let senior = Trait::new("senior")
            .description("5+ years experience")
            .rate_multiplier(1.3);

        assert_eq!(senior.id, "senior");
        assert_eq!(senior.name, "senior");
        assert_eq!(senior.description, Some("5+ years experience".into()));
        assert_eq!(senior.rate_multiplier, 1.3);
    }

    #[test]
    fn trait_default_multiplier() {
        let t = Trait::new("generic");
        assert_eq!(t.rate_multiplier, 1.0);
    }

    #[test]
    fn rate_range_expected_midpoint() {
        use rust_decimal::Decimal;
        let range = RateRange::new(Decimal::from(450), Decimal::from(700));
        assert_eq!(range.expected(), Decimal::from(575));
    }

    #[test]
    fn rate_range_spread_percent() {
        use rust_decimal::Decimal;
        // Range 500-700, expected 600, spread = 200, spread% = (200/600)*100 = 33.33%
        let range = RateRange::new(Decimal::from(500), Decimal::from(700));
        let spread = range.spread_percent();
        assert!((spread - 33.33).abs() < 0.1);
    }

    #[test]
    fn rate_range_collapsed() {
        use rust_decimal::Decimal;
        let range = RateRange::new(Decimal::from(500), Decimal::from(500));
        assert!(range.is_collapsed());
        assert_eq!(range.spread_percent(), 0.0);
    }

    #[test]
    fn rate_range_inverted() {
        use rust_decimal::Decimal;
        let range = RateRange::new(Decimal::from(700), Decimal::from(500));
        assert!(range.is_inverted());
    }

    #[test]
    fn rate_range_apply_multiplier() {
        use rust_decimal::Decimal;
        let range = RateRange::new(Decimal::from(500), Decimal::from(700));
        let scaled = range.apply_multiplier(1.3);

        assert_eq!(scaled.min, Decimal::from(650));
        assert_eq!(scaled.max, Decimal::from(910));
    }

    #[test]
    fn rate_range_with_currency() {
        use rust_decimal::Decimal;
        let range = RateRange::new(Decimal::from(500), Decimal::from(700))
            .currency("EUR");

        assert_eq!(range.currency, Some("EUR".into()));
    }

    #[test]
    fn resource_rate_fixed() {
        use rust_decimal::Decimal;
        let rate = ResourceRate::Fixed(Money::new(Decimal::from(500), "USD"));

        assert!(rate.is_fixed());
        assert!(!rate.is_range());
        assert_eq!(rate.expected(), Decimal::from(500));
    }

    #[test]
    fn resource_rate_range() {
        use rust_decimal::Decimal;
        let rate = ResourceRate::Range(RateRange::new(Decimal::from(450), Decimal::from(700)));

        assert!(rate.is_range());
        assert!(!rate.is_fixed());
        assert_eq!(rate.expected(), Decimal::from(575));
    }

    #[test]
    fn resource_profile_builder() {
        use rust_decimal::Decimal;
        let profile = ResourceProfile::new("backend_dev")
            .name("Backend Developer")
            .description("Server-side development")
            .specializes("developer")
            .skill("java")
            .skill("sql")
            .rate_range(RateRange::new(Decimal::from(550), Decimal::from(800)));

        assert_eq!(profile.id, "backend_dev");
        assert_eq!(profile.name, "Backend Developer");
        assert_eq!(profile.description, Some("Server-side development".into()));
        assert_eq!(profile.specializes, Some("developer".into()));
        assert_eq!(profile.skills, vec!["java", "sql"]);
        assert!(profile.rate.is_some());
    }

    #[test]
    fn resource_profile_with_traits() {
        let profile = ResourceProfile::new("senior_dev")
            .with_trait("senior")
            .with_traits(["contractor", "remote"]);

        assert_eq!(profile.traits.len(), 3);
        assert!(profile.traits.contains(&"senior".into()));
        assert!(profile.traits.contains(&"contractor".into()));
        assert!(profile.traits.contains(&"remote".into()));
    }

    #[test]
    fn resource_profile_is_abstract() {
        use rust_decimal::Decimal;
        // Profile with no rate is abstract
        let no_rate = ResourceProfile::new("dev");
        assert!(no_rate.is_abstract());

        // Profile with range is abstract
        let with_range = ResourceProfile::new("dev")
            .rate_range(RateRange::new(Decimal::from(500), Decimal::from(700)));
        assert!(with_range.is_abstract());

        // Profile with fixed rate is concrete
        let with_fixed = ResourceProfile::new("dev")
            .rate(Money::new(Decimal::from(600), "USD"));
        assert!(!with_fixed.is_abstract());
    }

    #[test]
    fn resource_profile_skills_batch() {
        let profile = ResourceProfile::new("dev")
            .skills(["rust", "python", "go"]);

        assert_eq!(profile.skills.len(), 3);
        assert!(profile.skills.contains(&"rust".into()));
    }

    #[test]
    fn cost_range_fixed() {
        use rust_decimal::Decimal;
        let cost = CostRange::fixed(Decimal::from(50000), "USD");

        assert!(cost.is_fixed());
        assert_eq!(cost.spread_percent(), 0.0);
        assert_eq!(cost.min, cost.max);
        assert_eq!(cost.expected, cost.min);
    }

    #[test]
    fn cost_range_spread() {
        use rust_decimal::Decimal;
        // Cost range $40,000 - $60,000, expected $50,000
        // Half spread = $10,000, spread% = (10000/50000)*100 = 20%
        let cost = CostRange::new(
            Decimal::from(40000),
            Decimal::from(50000),
            Decimal::from(60000),
            "USD",
        );

        let spread = cost.spread_percent();
        assert!((spread - 20.0).abs() < 0.1);
    }

    #[test]
    fn cost_range_add() {
        use rust_decimal::Decimal;
        let cost1 = CostRange::new(
            Decimal::from(10000),
            Decimal::from(15000),
            Decimal::from(20000),
            "USD",
        );
        let cost2 = CostRange::new(
            Decimal::from(5000),
            Decimal::from(7500),
            Decimal::from(10000),
            "USD",
        );

        let total = cost1.add(&cost2);
        assert_eq!(total.min, Decimal::from(15000));
        assert_eq!(total.expected, Decimal::from(22500));
        assert_eq!(total.max, Decimal::from(30000));
    }

    #[test]
    fn cost_policy_midpoint() {
        use rust_decimal::Decimal;
        let policy = CostPolicy::Midpoint;
        let expected = policy.expected(Decimal::from(100), Decimal::from(200));
        assert_eq!(expected, Decimal::from(150));
    }

    #[test]
    fn cost_policy_optimistic() {
        use rust_decimal::Decimal;
        let policy = CostPolicy::Optimistic;
        let expected = policy.expected(Decimal::from(100), Decimal::from(200));
        assert_eq!(expected, Decimal::from(100));
    }

    #[test]
    fn cost_policy_pessimistic() {
        use rust_decimal::Decimal;
        let policy = CostPolicy::Pessimistic;
        let expected = policy.expected(Decimal::from(100), Decimal::from(200));
        assert_eq!(expected, Decimal::from(200));
    }

    #[test]
    fn cost_policy_default_is_midpoint() {
        assert_eq!(CostPolicy::default(), CostPolicy::Midpoint);
    }

    #[test]
    fn resource_specializes() {
        let resource = Resource::new("alice")
            .name("Alice")
            .specializes("backend_senior")
            .availability(0.8);

        assert_eq!(resource.specializes, Some("backend_senior".into()));
        assert_eq!(resource.availability, Some(0.8));
        assert!(resource.is_specialized());
    }

    #[test]
    fn resource_effective_availability() {
        let full_time = Resource::new("dev1");
        assert_eq!(full_time.effective_availability(), 1.0);

        let part_time = Resource::new("dev2").availability(0.5);
        assert_eq!(part_time.effective_availability(), 0.5);
    }

    #[test]
    fn project_get_profile() {
        use rust_decimal::Decimal;
        let mut project = Project::new("Test");
        project.profiles.push(
            ResourceProfile::new("developer")
                .rate_range(RateRange::new(Decimal::from(500), Decimal::from(700)))
        );
        project.profiles.push(
            ResourceProfile::new("designer")
                .rate_range(RateRange::new(Decimal::from(400), Decimal::from(600)))
        );

        let dev = project.get_profile("developer");
        assert!(dev.is_some());
        assert_eq!(dev.unwrap().id, "developer");

        let missing = project.get_profile("manager");
        assert!(missing.is_none());
    }

    #[test]
    fn project_get_trait() {
        let mut project = Project::new("Test");
        project.traits.push(Trait::new("senior").rate_multiplier(1.3));
        project.traits.push(Trait::new("junior").rate_multiplier(0.8));

        let senior = project.get_trait("senior");
        assert!(senior.is_some());
        assert_eq!(senior.unwrap().rate_multiplier, 1.3);

        let missing = project.get_trait("contractor");
        assert!(missing.is_none());
    }

    #[test]
    fn project_has_rfc0001_fields() {
        let project = Project::new("Test");

        // New fields should be initialized
        assert!(project.profiles.is_empty());
        assert!(project.traits.is_empty());
        assert_eq!(project.cost_policy, CostPolicy::Midpoint);
    }

    // ========================================================================
    // Diagnostic Tests
    // ========================================================================

    #[test]
    fn severity_as_str() {
        assert_eq!(Severity::Error.as_str(), "error");
        assert_eq!(Severity::Warning.as_str(), "warning");
        assert_eq!(Severity::Hint.as_str(), "hint");
        assert_eq!(Severity::Info.as_str(), "info");
    }

    #[test]
    fn severity_display() {
        assert_eq!(format!("{}", Severity::Error), "error");
        assert_eq!(format!("{}", Severity::Warning), "warning");
    }

    #[test]
    fn diagnostic_code_as_str() {
        assert_eq!(DiagnosticCode::E001CircularSpecialization.as_str(), "E001");
        assert_eq!(DiagnosticCode::W001AbstractAssignment.as_str(), "W001");
        assert_eq!(DiagnosticCode::H001MixedAbstraction.as_str(), "H001");
        assert_eq!(DiagnosticCode::I001ProjectCostSummary.as_str(), "I001");
    }

    #[test]
    fn diagnostic_code_default_severity() {
        assert_eq!(
            DiagnosticCode::E001CircularSpecialization.default_severity(),
            Severity::Error
        );
        assert_eq!(
            DiagnosticCode::W001AbstractAssignment.default_severity(),
            Severity::Warning
        );
        assert_eq!(
            DiagnosticCode::H001MixedAbstraction.default_severity(),
            Severity::Hint
        );
        assert_eq!(
            DiagnosticCode::I001ProjectCostSummary.default_severity(),
            Severity::Info
        );
    }

    #[test]
    fn diagnostic_code_ordering_priority() {
        // Errors come before warnings
        assert!(
            DiagnosticCode::E001CircularSpecialization.ordering_priority()
                < DiagnosticCode::W001AbstractAssignment.ordering_priority()
        );
        // Warnings come before hints
        assert!(
            DiagnosticCode::W001AbstractAssignment.ordering_priority()
                < DiagnosticCode::H001MixedAbstraction.ordering_priority()
        );
        // Hints come before info
        assert!(
            DiagnosticCode::H001MixedAbstraction.ordering_priority()
                < DiagnosticCode::I001ProjectCostSummary.ordering_priority()
        );
    }

    #[test]
    fn diagnostic_new_derives_severity() {
        let d = Diagnostic::new(
            DiagnosticCode::W001AbstractAssignment,
            "test message",
        );
        assert_eq!(d.severity, Severity::Warning);
        assert_eq!(d.code, DiagnosticCode::W001AbstractAssignment);
        assert_eq!(d.message, "test message");
    }

    #[test]
    fn diagnostic_builder_pattern() {
        let d = Diagnostic::new(DiagnosticCode::W001AbstractAssignment, "test")
            .with_file("test.proj")
            .with_span(SourceSpan::new(10, 5, 15))
            .with_note("additional info")
            .with_hint("try this instead");

        assert_eq!(d.file, Some(std::path::PathBuf::from("test.proj")));
        assert!(d.span.is_some());
        assert_eq!(d.span.as_ref().unwrap().line, 10);
        assert_eq!(d.notes.len(), 1);
        assert_eq!(d.hints.len(), 1);
    }

    #[test]
    fn diagnostic_is_error() {
        let error = Diagnostic::error(DiagnosticCode::E001CircularSpecialization, "cycle");
        let warning = Diagnostic::warning(DiagnosticCode::W001AbstractAssignment, "abstract");

        assert!(error.is_error());
        assert!(!error.is_warning());
        assert!(!warning.is_error());
        assert!(warning.is_warning());
    }

    #[test]
    fn source_span_with_label() {
        let span = SourceSpan::new(5, 10, 8).with_label("here");
        assert_eq!(span.line, 5);
        assert_eq!(span.column, 10);
        assert_eq!(span.length, 8);
        assert_eq!(span.label, Some("here".to_string()));
    }

    #[test]
    fn collecting_emitter_basic() {
        let mut emitter = CollectingEmitter::new();

        emitter.emit(Diagnostic::error(DiagnosticCode::E001CircularSpecialization, "error1"));
        emitter.emit(Diagnostic::warning(DiagnosticCode::W001AbstractAssignment, "warn1"));
        emitter.emit(Diagnostic::warning(DiagnosticCode::W002WideCostRange, "warn2"));

        assert_eq!(emitter.diagnostics.len(), 3);
        assert!(emitter.has_errors());
        assert_eq!(emitter.error_count(), 1);
        assert_eq!(emitter.warning_count(), 2);
    }

    #[test]
    fn collecting_emitter_sorted() {
        let mut emitter = CollectingEmitter::new();

        // Emit in wrong order
        emitter.emit(Diagnostic::new(DiagnosticCode::I001ProjectCostSummary, "info"));
        emitter.emit(Diagnostic::new(DiagnosticCode::W001AbstractAssignment, "warn"));
        emitter.emit(Diagnostic::new(DiagnosticCode::E001CircularSpecialization, "error"));
        emitter.emit(Diagnostic::new(DiagnosticCode::H001MixedAbstraction, "hint"));

        let sorted = emitter.sorted();

        // Should be: Error, Warning, Hint, Info
        assert_eq!(sorted[0].code, DiagnosticCode::E001CircularSpecialization);
        assert_eq!(sorted[1].code, DiagnosticCode::W001AbstractAssignment);
        assert_eq!(sorted[2].code, DiagnosticCode::H001MixedAbstraction);
        assert_eq!(sorted[3].code, DiagnosticCode::I001ProjectCostSummary);
    }

    #[test]
    fn collecting_emitter_sorted_by_location() {
        let mut emitter = CollectingEmitter::new();

        // Same code, different locations
        emitter.emit(
            Diagnostic::new(DiagnosticCode::W001AbstractAssignment, "second")
                .with_file("a.proj")
                .with_span(SourceSpan::new(20, 1, 5))
        );
        emitter.emit(
            Diagnostic::new(DiagnosticCode::W001AbstractAssignment, "first")
                .with_file("a.proj")
                .with_span(SourceSpan::new(10, 1, 5))
        );

        let sorted = emitter.sorted();

        assert_eq!(sorted[0].message, "first");
        assert_eq!(sorted[1].message, "second");
    }

    #[test]
    fn diagnostic_code_as_str_all_codes() {
        // Test all diagnostic codes have correct string representation
        assert_eq!(DiagnosticCode::E002ProfileWithoutRate.as_str(), "E002");
        assert_eq!(DiagnosticCode::E003InfeasibleConstraint.as_str(), "E003");
        assert_eq!(DiagnosticCode::W002WideCostRange.as_str(), "W002");
        assert_eq!(DiagnosticCode::W003UnknownTrait.as_str(), "W003");
        assert_eq!(DiagnosticCode::W004ApproximateLeveling.as_str(), "W004");
        assert_eq!(DiagnosticCode::W005ConstraintZeroSlack.as_str(), "W005");
        assert_eq!(DiagnosticCode::W006ScheduleVariance.as_str(), "W006");
        assert_eq!(DiagnosticCode::W014ContainerDependency.as_str(), "W014");
        assert_eq!(DiagnosticCode::H002UnusedProfile.as_str(), "H002");
        assert_eq!(DiagnosticCode::H003UnusedTrait.as_str(), "H003");
        assert_eq!(DiagnosticCode::H004TaskUnconstrained.as_str(), "H004");
        assert_eq!(DiagnosticCode::I002RefinementProgress.as_str(), "I002");
        assert_eq!(DiagnosticCode::I003ResourceUtilization.as_str(), "I003");
        assert_eq!(DiagnosticCode::I004ProjectStatus.as_str(), "I004");
        assert_eq!(DiagnosticCode::I005EarnedValueSummary.as_str(), "I005");
    }

    #[test]
    fn diagnostic_code_default_severity_all() {
        // Errors
        assert_eq!(DiagnosticCode::E003InfeasibleConstraint.default_severity(), Severity::Error);
        // Warnings (W002 onwards - E002 is warning by default, error in strict)
        assert_eq!(DiagnosticCode::E002ProfileWithoutRate.default_severity(), Severity::Warning);
        assert_eq!(DiagnosticCode::W004ApproximateLeveling.default_severity(), Severity::Warning);
        assert_eq!(DiagnosticCode::W005ConstraintZeroSlack.default_severity(), Severity::Warning);
        assert_eq!(DiagnosticCode::W006ScheduleVariance.default_severity(), Severity::Warning);
        // Hints
        assert_eq!(DiagnosticCode::H002UnusedProfile.default_severity(), Severity::Hint);
        assert_eq!(DiagnosticCode::H003UnusedTrait.default_severity(), Severity::Hint);
        assert_eq!(DiagnosticCode::H004TaskUnconstrained.default_severity(), Severity::Hint);
        // Info
        assert_eq!(DiagnosticCode::I002RefinementProgress.default_severity(), Severity::Info);
        assert_eq!(DiagnosticCode::I003ResourceUtilization.default_severity(), Severity::Info);
        assert_eq!(DiagnosticCode::I004ProjectStatus.default_severity(), Severity::Info);
        assert_eq!(DiagnosticCode::I005EarnedValueSummary.default_severity(), Severity::Info);
    }

    #[test]
    fn diagnostic_code_ordering_priority_all() {
        // Errors have lowest priority (emitted first)
        assert!(DiagnosticCode::E002ProfileWithoutRate.ordering_priority() < 10);
        assert!(DiagnosticCode::E003InfeasibleConstraint.ordering_priority() < 10);
        // Cost warnings
        assert_eq!(DiagnosticCode::W002WideCostRange.ordering_priority(), 10);
        assert_eq!(DiagnosticCode::W004ApproximateLeveling.ordering_priority(), 11);
        assert_eq!(DiagnosticCode::W005ConstraintZeroSlack.ordering_priority(), 12);
        assert_eq!(DiagnosticCode::W006ScheduleVariance.ordering_priority(), 13);
        assert_eq!(DiagnosticCode::W014ContainerDependency.ordering_priority(), 14);
        // Assignment warnings
        assert_eq!(DiagnosticCode::W003UnknownTrait.ordering_priority(), 21);
        // Hints
        assert_eq!(DiagnosticCode::H002UnusedProfile.ordering_priority(), 31);
        assert_eq!(DiagnosticCode::H003UnusedTrait.ordering_priority(), 32);
        assert_eq!(DiagnosticCode::H004TaskUnconstrained.ordering_priority(), 33);
        // Info (highest priority = emitted last)
        assert_eq!(DiagnosticCode::I002RefinementProgress.ordering_priority(), 41);
        assert_eq!(DiagnosticCode::I003ResourceUtilization.ordering_priority(), 42);
        assert_eq!(DiagnosticCode::I004ProjectStatus.ordering_priority(), 43);
        assert_eq!(DiagnosticCode::I005EarnedValueSummary.ordering_priority(), 44);
    }

    #[test]
    fn diagnostic_code_display() {
        // Test Display trait implementation
        assert_eq!(format!("{}", DiagnosticCode::E001CircularSpecialization), "E001");
        assert_eq!(format!("{}", DiagnosticCode::W014ContainerDependency), "W014");
        assert_eq!(format!("{}", DiagnosticCode::H004TaskUnconstrained), "H004");
    }

    #[test]
    fn rate_range_spread_percent_zero_expected() {
        use rust_decimal::Decimal;
        // When min == max == 0, spread should be 0% (not NaN or error)
        let range = RateRange::new(Decimal::ZERO, Decimal::ZERO);
        assert_eq!(range.spread_percent(), 0.0);
    }

    #[test]
    fn cost_range_spread_percent_zero_expected() {
        use rust_decimal::Decimal;
        // When expected is zero, spread should be 0% (not NaN or error)
        let range = CostRange::new(Decimal::ZERO, Decimal::ZERO, Decimal::ZERO, "USD");
        assert_eq!(range.spread_percent(), 0.0);
    }

    #[test]
    fn resource_profile_builder_calendar() {
        let profile = ResourceProfile::new("dev")
            .calendar("work_calendar");
        assert_eq!(profile.calendar, Some("work_calendar".to_string()));
    }

    #[test]
    fn resource_profile_builder_efficiency() {
        let profile = ResourceProfile::new("dev")
            .efficiency(0.8);
        assert_eq!(profile.efficiency, Some(0.8));
    }

    #[test]
    fn task_builder_summary() {
        let task = Task::new("task1").summary("Short name");
        assert_eq!(task.summary, Some("Short name".to_string()));
    }

    #[test]
    fn task_builder_effort() {
        let task = Task::new("task1").effort(Duration::days(5));
        assert_eq!(task.effort, Some(Duration::days(5)));
    }

    #[test]
    fn task_builder_duration() {
        let task = Task::new("task1").duration(Duration::days(3));
        assert_eq!(task.duration, Some(Duration::days(3)));
    }

    #[test]
    fn task_builder_depends_on() {
        let task = Task::new("task2").depends_on("task1");
        assert_eq!(task.depends.len(), 1);
        assert_eq!(task.depends[0].predecessor, "task1");
        assert_eq!(task.depends[0].dep_type, DependencyType::FinishToStart);
        assert_eq!(task.depends[0].lag, None);
    }

    #[test]
    fn task_builder_assign() {
        let task = Task::new("task1").assign("alice");
        assert_eq!(task.assigned.len(), 1);
        assert_eq!(task.assigned[0].resource_id, "alice");
        assert_eq!(task.assigned[0].units, 1.0);
    }

    #[test]
    fn task_builder_assign_with_units() {
        let task = Task::new("task1").assign_with_units("bob", 0.5);
        assert_eq!(task.assigned.len(), 1);
        assert_eq!(task.assigned[0].resource_id, "bob");
        assert_eq!(task.assigned[0].units, 0.5);
    }

    #[test]
    fn source_span_with_label_and_display() {
        let span = SourceSpan::new(10, 5, 8).with_label("highlight");
        assert_eq!(span.line, 10);
        assert_eq!(span.column, 5);
        assert_eq!(span.length, 8);
        assert_eq!(span.label, Some("highlight".to_string()));
    }

    // =========================================================================
    // Scheduling Mode Tests
    // =========================================================================

    #[test]
    fn scheduling_mode_default_is_duration_based() {
        assert_eq!(SchedulingMode::default(), SchedulingMode::DurationBased);
    }

    #[test]
    fn scheduling_mode_description() {
        assert_eq!(
            SchedulingMode::DurationBased.description(),
            "duration-based (no effort tracking)"
        );
        assert_eq!(
            SchedulingMode::EffortBased.description(),
            "effort-based (no cost tracking)"
        );
        assert_eq!(
            SchedulingMode::ResourceLoaded.description(),
            "resource-loaded (full tracking)"
        );
    }

    #[test]
    fn scheduling_mode_display() {
        assert_eq!(
            format!("{}", SchedulingMode::DurationBased),
            "duration-based (no effort tracking)"
        );
        assert_eq!(
            format!("{}", SchedulingMode::ResourceLoaded),
            "resource-loaded (full tracking)"
        );
    }

    #[test]
    fn scheduling_mode_capabilities_duration_based() {
        let caps = SchedulingMode::DurationBased.capabilities();
        assert!(caps.timeline);
        assert!(!caps.utilization);
        assert!(!caps.cost_tracking);
    }

    #[test]
    fn scheduling_mode_capabilities_effort_based() {
        let caps = SchedulingMode::EffortBased.capabilities();
        assert!(caps.timeline);
        assert!(caps.utilization);
        assert!(!caps.cost_tracking);
    }

    #[test]
    fn scheduling_mode_capabilities_resource_loaded() {
        let caps = SchedulingMode::ResourceLoaded.capabilities();
        assert!(caps.timeline);
        assert!(caps.utilization);
        assert!(caps.cost_tracking);
    }
}
