//! Interactive Tours and Onboarding
//! 
//! Provides guided tours for new users with visual aids and step-by-step instructions
//! to understand leverage trading and platform features.

use solana_program::{
    account_info::{AccountInfo, next_account_info},
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
};

use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    account_validation::DISCRIMINATOR_SIZE,
    error::BettingPlatformError,
    state::accounts::discriminators,
    events::{EventType, Event},
    define_event,
};

/// Tour types available
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TourType {
    /// Basic platform introduction
    BasicIntro,
    /// Understanding leverage
    LeverageBasics,
    /// Risk management tour
    RiskManagement,
    /// Chain positions tutorial
    ChainPositions,
    /// Advanced features
    AdvancedFeatures,
    /// Migration from Polymarket
    MigrationGuide,
}

impl TourType {
    pub fn get_steps(&self) -> Vec<TourStep> {
        match self {
            TourType::BasicIntro => vec![
                TourStep {
                    id: 1,
                    title: "Welcome to the Platform!".to_string(),
                    description: "Let's learn the key concepts that make us different".to_string(),
                    action: TourAction::ShowTooltip("Click next to continue".to_string()),
                    gif_url: Some("/assets/tours/welcome.gif".to_string()),
                    highlight_element: None,
                    requires_action: false,
                    min_duration_ms: 3000,
                },
                TourStep {
                    id: 2,
                    title: "Understanding Verses".to_string(),
                    description: "Verses = bet groups. Think of them as different betting universes!".to_string(),
                    action: TourAction::ShowTooltip("Each verse groups related markets together".to_string()),
                    gif_url: Some("/assets/tours/verses-explained.gif".to_string()),
                    highlight_element: Some("verse-selector".to_string()),
                    requires_action: false,
                    min_duration_ms: 4000,
                },
                TourStep {
                    id: 3,
                    title: "What are Chains?".to_string(),
                    description: "Chains = auto-boost. Your profits automatically increase your next position!".to_string(),
                    action: TourAction::ShowTooltip("Win → Auto-reinvest → Bigger wins! (+400% efficiency)".to_string()),
                    gif_url: Some("/assets/tours/chains-autoboost.gif".to_string()),
                    highlight_element: Some("chain-indicator".to_string()),
                    requires_action: false,
                    min_duration_ms: 4000,
                },
                TourStep {
                    id: 4,
                    title: "Quantum Mode".to_string(),
                    description: "Quantum = test ideas cheap. Try strategies with minimal risk!".to_string(),
                    action: TourAction::ShowTooltip("Small bets, big insights".to_string()),
                    gif_url: Some("/assets/tours/quantum-testing.gif".to_string()),
                    highlight_element: Some("quantum-mode-toggle".to_string()),
                    requires_action: false,
                    min_duration_ms: 4000,
                },
                TourStep {
                    id: 5,
                    title: "Browse Markets".to_string(),
                    description: "Now let's explore available prediction markets".to_string(),
                    action: TourAction::ShowTooltip("Click 'Markets' to browse".to_string()),
                    gif_url: Some("/assets/tours/browse-markets.gif".to_string()),
                    highlight_element: Some("markets-nav-button".to_string()),
                    requires_action: false,
                    min_duration_ms: 3000,
                },
                TourStep {
                    id: 6,
                    title: "Choose a Market".to_string(),
                    description: "Select any market that interests you".to_string(),
                    action: TourAction::WaitForClick("market-card".to_string()),
                    gif_url: Some("/assets/tours/select-market.gif".to_string()),
                    highlight_element: Some("market-card".to_string()),
                    requires_action: true,
                    min_duration_ms: 2000,
                },
                TourStep {
                    id: 7,
                    title: "Place Your First Trade".to_string(),
                    description: "Enter amount and choose YES or NO".to_string(),
                    action: TourAction::ShowForm(vec!["amount-input".to_string(), "yes-button".to_string(), "no-button".to_string()]),
                    gif_url: Some("/assets/tours/place-trade.gif".to_string()),
                    highlight_element: Some("trade-form".to_string()),
                    requires_action: true,
                    min_duration_ms: 5000,
                },
            ],
            
            TourType::LeverageBasics => vec![
                TourStep {
                    id: 1,
                    title: "What is Leverage?".to_string(),
                    description: "Leverage multiplies your position size AND your risk".to_string(),
                    action: TourAction::ShowAnimation("leverage-explainer".to_string()),
                    gif_url: Some("/assets/tours/leverage-basics.gif".to_string()),
                    highlight_element: None,
                    requires_action: false,
                    min_duration_ms: 8000,
                },
                TourStep {
                    id: 2,
                    title: "Leverage Slider".to_string(),
                    description: "Try adjusting leverage and see how it affects your position".to_string(),
                    action: TourAction::InteractiveDemo("leverage-slider-demo".to_string()),
                    gif_url: Some("/assets/tours/leverage-slider.gif".to_string()),
                    highlight_element: Some("leverage-slider".to_string()),
                    requires_action: true,
                    min_duration_ms: 5000,
                },
                TourStep {
                    id: 3,
                    title: "Liquidation Warning".to_string(),
                    description: "Higher leverage = closer liquidation price!".to_string(),
                    action: TourAction::ShowWarning("With 100x leverage, a 1% move liquidates you!".to_string()),
                    gif_url: Some("/assets/tours/liquidation-warning.gif".to_string()),
                    highlight_element: Some("liquidation-price-display".to_string()),
                    requires_action: false,
                    min_duration_ms: 6000,
                },
            ],
            
            TourType::RiskManagement => vec![
                TourStep {
                    id: 1,
                    title: "Health Monitoring".to_string(),
                    description: "Keep an eye on your position health bar".to_string(),
                    action: TourAction::ShowTooltip("Green = Safe, Yellow = Caution, Red = Danger".to_string()),
                    gif_url: Some("/assets/tours/health-bar.gif".to_string()),
                    highlight_element: Some("position-health-bar".to_string()),
                    requires_action: false,
                    min_duration_ms: 4000,
                },
                TourStep {
                    id: 2,
                    title: "Stop Loss Orders".to_string(),
                    description: "Protect yourself with automatic exit orders".to_string(),
                    action: TourAction::ShowForm(vec!["stop-loss-input".to_string()]),
                    gif_url: Some("/assets/tours/stop-loss.gif".to_string()),
                    highlight_element: Some("stop-loss-section".to_string()),
                    requires_action: false,
                    min_duration_ms: 5000,
                },
                TourStep {
                    id: 3,
                    title: "Demo Mode".to_string(),
                    description: "Practice with fake money first!".to_string(),
                    action: TourAction::NavigateTo("/demo".to_string()),
                    gif_url: Some("/assets/tours/demo-mode.gif".to_string()),
                    highlight_element: Some("demo-mode-toggle".to_string()),
                    requires_action: false,
                    min_duration_ms: 4000,
                },
            ],
            
            _ => vec![], // Other tours not implemented yet
        }
    }
    
    pub fn get_duration_ms(&self) -> u64 {
        self.get_steps().iter().map(|s| s.min_duration_ms).sum()
    }
    
    pub fn get_difficulty(&self) -> TourDifficulty {
        match self {
            TourType::BasicIntro => TourDifficulty::Beginner,
            TourType::LeverageBasics => TourDifficulty::Intermediate,
            TourType::RiskManagement => TourDifficulty::Intermediate,
            TourType::ChainPositions => TourDifficulty::Advanced,
            TourType::AdvancedFeatures => TourDifficulty::Expert,
            TourType::MigrationGuide => TourDifficulty::Beginner,
        }
    }
}

/// Difficulty levels for tours
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TourDifficulty {
    Beginner,
    Intermediate,
    Advanced,
    Expert,
}

/// Individual tour step
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct TourStep {
    /// Step ID
    pub id: u8,
    /// Step title
    pub title: String,
    /// Step description
    pub description: String,
    /// Action to perform
    pub action: TourAction,
    /// Optional GIF URL for visual aid
    pub gif_url: Option<String>,
    /// Element to highlight
    pub highlight_element: Option<String>,
    /// Whether user action is required to proceed
    pub requires_action: bool,
    /// Minimum duration before allowing next step
    pub min_duration_ms: u64,
}

/// Tour actions
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum TourAction {
    /// Show a tooltip with text
    ShowTooltip(String),
    /// Wait for user to click element
    WaitForClick(String),
    /// Show a form with fields
    ShowForm(Vec<String>),
    /// Show an animation
    ShowAnimation(String),
    /// Interactive demo
    InteractiveDemo(String),
    /// Show warning message
    ShowWarning(String),
    /// Navigate to URL
    NavigateTo(String),
    /// Custom action
    Custom(String),
}

/// User's tour progress
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct TourProgress {
    /// Account discriminator
    pub discriminator: [u8; DISCRIMINATOR_SIZE],
    /// User pubkey
    pub user: Pubkey,
    /// Tours completed
    pub completed_tours: Vec<TourType>,
    /// Current tour (if any)
    pub current_tour: Option<TourType>,
    /// Current step in tour
    pub current_step: u8,
    /// Tours started but not completed
    pub started_tours: Vec<(TourType, u8)>, // (tour_type, last_step)
    /// Total tours completed
    pub tours_completed_count: u32,
    /// Achievement badges earned
    pub badges_earned: Vec<TourBadge>,
    /// Last tour activity
    pub last_activity: i64,
}

impl TourProgress {
    pub const SIZE: usize = 8 + // discriminator
        32 + // user
        1 + 10 + // completed_tours (Option + max 10 tours)
        1 + 1 + // current_tour
        1 + // current_step
        1 + 20 + // started_tours (max 10 tours * 2 bytes each)
        4 + // tours_completed_count
        1 + 10 + // badges_earned (max 10 badges)
        8; // last_activity

    pub fn new(user: Pubkey) -> Self {
        Self {
            discriminator: discriminators::USER_STATS, // Reuse existing discriminator
            user,
            completed_tours: Vec::new(),
            current_tour: None,
            current_step: 0,
            started_tours: Vec::new(),
            tours_completed_count: 0,
            badges_earned: Vec::new(),
            last_activity: Clock::get().unwrap_or_default().unix_timestamp,
        }
    }

    pub fn start_tour(&mut self, tour_type: TourType) -> Result<(), ProgramError> {
        if self.current_tour.is_some() {
            return Err(BettingPlatformError::TourInProgress.into());
        }

        self.current_tour = Some(tour_type);
        self.current_step = 1;
        self.last_activity = Clock::get()?.unix_timestamp;

        Ok(())
    }

    pub fn advance_step(&mut self) -> Result<Option<TourStep>, ProgramError> {
        match self.current_tour {
            Some(tour) => {
                let steps = tour.get_steps();
                
                if (self.current_step as usize) < steps.len() {
                    self.current_step += 1;
                    self.last_activity = Clock::get()?.unix_timestamp;
                    
                    if (self.current_step as usize) <= steps.len() {
                        Ok(Some(steps[(self.current_step - 1) as usize].clone()))
                    } else {
                        // Tour completed
                        self.complete_current_tour()?;
                        Ok(None)
                    }
                } else {
                    self.complete_current_tour()?;
                    Ok(None)
                }
            }
            None => Err(BettingPlatformError::NoActiveTour.into()),
        }
    }

    pub fn complete_current_tour(&mut self) -> Result<(), ProgramError> {
        if let Some(tour) = self.current_tour.take() {
            if !self.completed_tours.contains(&tour) {
                self.completed_tours.push(tour);
                self.tours_completed_count += 1;
                
                // Award badges
                self.check_and_award_badges()?;
            }
            
            // Remove from started tours
            self.started_tours.retain(|(t, _)| *t != tour);
            
            self.current_tour = None;
            self.current_step = 0;
            self.last_activity = Clock::get()?.unix_timestamp;
        }
        
        Ok(())
    }

    pub fn pause_current_tour(&mut self) -> Result<(), ProgramError> {
        if let Some(tour) = self.current_tour.take() {
            // Save progress
            self.started_tours.push((tour, self.current_step));
            self.current_step = 0;
            self.last_activity = Clock::get()?.unix_timestamp;
        }
        
        Ok(())
    }

    pub fn resume_tour(&mut self, tour_type: TourType) -> Result<u8, ProgramError> {
        if self.current_tour.is_some() {
            return Err(BettingPlatformError::TourInProgress.into());
        }

        // Find saved progress
        if let Some(idx) = self.started_tours.iter().position(|(t, _)| *t == tour_type) {
            let (_, last_step) = self.started_tours.remove(idx);
            self.current_tour = Some(tour_type);
            self.current_step = last_step;
            self.last_activity = Clock::get()?.unix_timestamp;
            Ok(last_step)
        } else {
            // Start fresh
            self.start_tour(tour_type)?;
            Ok(1)
        }
    }

    fn check_and_award_badges(&mut self) -> Result<(), ProgramError> {
        // First tour badge
        if self.tours_completed_count == 1 && !self.badges_earned.contains(&TourBadge::FirstSteps) {
            self.badges_earned.push(TourBadge::FirstSteps);
        }

        // All basic tours
        let basic_tours = vec![TourType::BasicIntro, TourType::LeverageBasics, TourType::RiskManagement];
        if basic_tours.iter().all(|t| self.completed_tours.contains(t)) 
            && !self.badges_earned.contains(&TourBadge::QuickLearner) {
            self.badges_earned.push(TourBadge::QuickLearner);
        }

        // All tours
        if self.tours_completed_count >= 6 && !self.badges_earned.contains(&TourBadge::TourMaster) {
            self.badges_earned.push(TourBadge::TourMaster);
        }

        Ok(())
    }
}

/// Tour achievement badges
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TourBadge {
    /// Completed first tour
    FirstSteps,
    /// Completed all basic tours
    QuickLearner,
    /// Completed risk management tour
    RiskAware,
    /// Completed all tours
    TourMaster,
    /// Used demo mode
    PracticeMakesPerfect,
}

/// Process tour start
pub fn process_start_tour(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    tour_type: TourType,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let user = next_account_info(account_info_iter)?;
    let progress_account = next_account_info(account_info_iter)?;
    
    // Validate signer
    if !user.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load or create progress
    let mut progress = if progress_account.data_is_empty() {
        TourProgress::new(*user.key)
    } else {
        TourProgress::try_from_slice(&progress_account.data.borrow())?
    };
    
    // Verify ownership
    if progress.user != *user.key {
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }
    
    // Start tour
    progress.start_tour(tour_type)?;
    
    // Save progress
    progress.serialize(&mut &mut progress_account.data.borrow_mut()[..])?;
    
    msg!("Tour started: {:?}", tour_type);
    
    // Emit event
    let event = TourStarted {
        user: *user.key,
        tour_type,
        timestamp: progress.last_activity,
    };
    event.emit();
    
    Ok(())
}

/// Process tour step advancement
pub fn process_advance_tour_step(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let user = next_account_info(account_info_iter)?;
    let progress_account = next_account_info(account_info_iter)?;
    
    // Validate signer
    if !user.is_signer {
        return Err(BettingPlatformError::Unauthorized.into());
    }
    
    // Load progress
    let mut progress = TourProgress::try_from_slice(&progress_account.data.borrow())?;
    
    // Verify ownership
    if progress.user != *user.key {
        return Err(BettingPlatformError::UnauthorizedAccess.into());
    }
    
    // Advance step
    let next_step = progress.advance_step()?;
    
    // Save progress
    progress.serialize(&mut &mut progress_account.data.borrow_mut()[..])?;
    
    if let Some(step) = next_step {
        msg!("Advanced to step {}: {}", step.id, step.title);
    } else {
        msg!("Tour completed!");
        
        // Emit completion event
        if let Some(completed_tour) = progress.completed_tours.last() {
            let event = TourCompleted {
                user: *user.key,
                tour_type: *completed_tour,
                timestamp: progress.last_activity,
            };
            event.emit();
        }
    }
    
    Ok(())
}

/// Get recommended tour for user
pub fn get_recommended_tour(progress: &TourProgress) -> Option<TourType> {
    // New user - start with basic intro
    if progress.completed_tours.is_empty() {
        return Some(TourType::BasicIntro);
    }
    
    // Completed basic intro - learn about leverage
    if progress.completed_tours.contains(&TourType::BasicIntro) 
        && !progress.completed_tours.contains(&TourType::LeverageBasics) {
        return Some(TourType::LeverageBasics);
    }
    
    // Learned leverage - understand risk
    if progress.completed_tours.contains(&TourType::LeverageBasics)
        && !progress.completed_tours.contains(&TourType::RiskManagement) {
        return Some(TourType::RiskManagement);
    }
    
    // Ready for advanced features
    if progress.completed_tours.len() >= 3 
        && !progress.completed_tours.contains(&TourType::ChainPositions) {
        return Some(TourType::ChainPositions);
    }
    
    None
}

// Event definitions
define_event!(TourStarted, EventType::DemoAccountCreated, {
    user: Pubkey,
    tour_type: TourType,
    timestamp: i64,
});

define_event!(TourCompleted, EventType::DemoAccountCreated, {
    user: Pubkey,
    tour_type: TourType,
    timestamp: i64,
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tour_steps() {
        let tour = TourType::BasicIntro;
        let steps = tour.get_steps();
        
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0].title, "Welcome to the Platform!");
        assert!(steps[0].gif_url.is_some());
    }

    #[test]
    fn test_tour_progress() {
        let user = Pubkey::new_unique();
        let mut progress = TourProgress::new(user);
        
        // Start tour
        progress.start_tour(TourType::BasicIntro).unwrap();
        assert_eq!(progress.current_tour, Some(TourType::BasicIntro));
        assert_eq!(progress.current_step, 1);
        
        // Advance steps
        progress.advance_step().unwrap();
        assert_eq!(progress.current_step, 2);
        
        progress.advance_step().unwrap();
        assert_eq!(progress.current_step, 3);
        
        // Complete tour
        progress.advance_step().unwrap();
        assert!(progress.current_tour.is_none());
        assert!(progress.completed_tours.contains(&TourType::BasicIntro));
    }

    #[test]
    fn test_badge_awarding() {
        let user = Pubkey::new_unique();
        let mut progress = TourProgress::new(user);
        
        // Complete first tour
        progress.start_tour(TourType::BasicIntro).unwrap();
        progress.complete_current_tour().unwrap();
        
        assert!(progress.badges_earned.contains(&TourBadge::FirstSteps));
    }

    #[test]
    fn test_tour_recommendations() {
        let user = Pubkey::new_unique();
        let mut progress = TourProgress::new(user);
        
        // New user should get basic intro
        assert_eq!(get_recommended_tour(&progress), Some(TourType::BasicIntro));
        
        // After basic intro, should get leverage basics
        progress.completed_tours.push(TourType::BasicIntro);
        assert_eq!(get_recommended_tour(&progress), Some(TourType::LeverageBasics));
    }
}