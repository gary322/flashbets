//! Access Control Framework
//!
//! Production-grade role-based access control and permission management

use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    clock::Clock,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::BettingPlatformError,
    state::accounts::discriminators,
};

/// Permission flags (bit flags for efficient storage)
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Permissions(pub u64);

impl Permissions {
    // Core permissions
    pub const NONE: Self = Self(0);
    pub const CREATE_PROPOSAL: Self = Self(1 << 0);
    pub const RESOLVE_PROPOSAL: Self = Self(1 << 1);
    pub const PAUSE_PROTOCOL: Self = Self(1 << 2);
    pub const MANAGE_FEES: Self = Self(1 << 3);
    pub const MANAGE_LIQUIDITY: Self = Self(1 << 4);
    pub const LIQUIDATE_POSITIONS: Self = Self(1 << 5);
    pub const EMERGENCY_WITHDRAW: Self = Self(1 << 6);
    pub const UPDATE_ORACLE: Self = Self(1 << 7);
    pub const MANAGE_KEEPERS: Self = Self(1 << 8);
    pub const UPDATE_CONFIG: Self = Self(1 << 9);
    pub const MANAGE_ROLES: Self = Self(1 << 10);
    pub const VIEW_PRIVATE_DATA: Self = Self(1 << 11);
    pub const EXECUTE_TRADES: Self = Self(1 << 12);
    pub const MANAGE_DARK_POOLS: Self = Self(1 << 13);
    pub const HALT_LIQUIDATIONS: Self = Self(1 << 14);
    
    // Combined permissions
    pub const TRADER: Self = Self(Self::CREATE_PROPOSAL.0 | Self::EXECUTE_TRADES.0);
    pub const KEEPER: Self = Self(Self::LIQUIDATE_POSITIONS.0 | Self::RESOLVE_PROPOSAL.0);
    pub const OPERATOR: Self = Self(
        Self::MANAGE_FEES.0 | 
        Self::MANAGE_LIQUIDITY.0 | 
        Self::UPDATE_ORACLE.0 |
        Self::MANAGE_KEEPERS.0
    );
    pub const ADMIN: Self = Self(u64::MAX); // All permissions
    
    /// Check if has permission
    pub fn has(&self, permission: Self) -> bool {
        (self.0 & permission.0) == permission.0
    }
    
    /// Add permission
    pub fn add(&mut self, permission: Self) {
        self.0 |= permission.0;
    }
    
    /// Remove permission
    pub fn remove(&mut self, permission: Self) {
        self.0 &= !permission.0;
    }
    
    /// Require permission
    pub fn require(&self, permission: Self) -> Result<(), ProgramError> {
        if !self.has(permission) {
            msg!("Missing required permission: {:b}", permission.0);
            return Err(BettingPlatformError::PermissionDenied.into());
        }
        Ok(())
    }
}

/// Role definition
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct Role {
    /// Role name (max 32 chars)
    pub name: [u8; 32],
    /// Role permissions
    pub permissions: Permissions,
    /// Maximum members (0 = unlimited)
    pub max_members: u16,
    /// Current member count
    pub member_count: u16,
    /// Role expiry (0 = never)
    pub expiry_slot: u64,
    /// Whether role is active
    pub is_active: bool,
}

impl Role {
    pub fn new(name: &str, permissions: Permissions) -> Self {
        let mut name_bytes = [0u8; 32];
        let name_slice = name.as_bytes();
        let len = name_slice.len().min(32);
        name_bytes[..len].copy_from_slice(&name_slice[..len]);
        
        Self {
            name: name_bytes,
            permissions,
            max_members: 0,
            member_count: 0,
            expiry_slot: 0,
            is_active: true,
        }
    }
    
    /// Check if role is valid
    pub fn is_valid(&self, current_slot: u64) -> bool {
        self.is_active && 
        (self.expiry_slot == 0 || current_slot < self.expiry_slot) &&
        (self.max_members == 0 || self.member_count < self.max_members)
    }
}

/// Access control list
#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct AccessControlList {
    /// Account discriminator
    pub discriminator: [u8; 8],
    /// ACL version
    pub version: u32,
    /// Protocol authority (super admin)
    pub authority: Pubkey,
    /// Roles (max 32)
    pub roles: Vec<Role>,
    /// User permissions mapping
    pub user_permissions: Vec<(Pubkey, Permissions)>,
    /// Role assignments
    pub role_assignments: Vec<(Pubkey, u8)>, // (user, role_index)
    /// Suspended users
    pub suspended_users: Vec<Pubkey>,
    /// Last update slot
    pub last_update_slot: u64,
}

impl AccessControlList {
    pub const MAX_ROLES: usize = 32;
    pub const MAX_USERS: usize = 1000;
    
    pub fn new(authority: Pubkey) -> Self {
        let mut acl = Self {
            discriminator: discriminators::ACCESS_CONTROL,
            version: 1,
            authority,
            roles: Vec::new(),
            user_permissions: Vec::new(),
            role_assignments: Vec::new(),
            suspended_users: Vec::new(),
            last_update_slot: 0,
        };
        
        // Initialize default roles
        acl.roles.push(Role::new("Admin", Permissions::ADMIN));
        acl.roles.push(Role::new("Operator", Permissions::OPERATOR));
        acl.roles.push(Role::new("Keeper", Permissions::KEEPER));
        acl.roles.push(Role::new("Trader", Permissions::TRADER));
        
        acl
    }
    
    /// Get user's effective permissions
    pub fn get_user_permissions(&self, user: &Pubkey) -> Permissions {
        let mut permissions = Permissions::NONE;
        
        // Direct permissions
        if let Some((_, perms)) = self.user_permissions.iter().find(|(u, _)| u == user) {
            permissions.0 |= perms.0;
        }
        
        // Role-based permissions
        for (assigned_user, role_index) in &self.role_assignments {
            if assigned_user == user {
                if let Some(role) = self.roles.get(*role_index as usize) {
                    if role.is_valid(Clock::get().unwrap_or_default().slot) {
                        permissions.0 |= role.permissions.0;
                    }
                }
            }
        }
        
        // Check if suspended
        if self.suspended_users.contains(user) {
            return Permissions::NONE;
        }
        
        permissions
    }
    
    /// Grant role to user
    pub fn grant_role(
        &mut self,
        user: &Pubkey,
        role_index: u8,
        authority: &Pubkey,
    ) -> Result<(), ProgramError> {
        // Check authority
        if *authority != self.authority {
            let auth_perms = self.get_user_permissions(authority);
            auth_perms.require(Permissions::MANAGE_ROLES)?;
        }
        
        // Validate role
        let role = self.roles.get_mut(role_index as usize)
            .ok_or(BettingPlatformError::InvalidRole)?;
        
        if !role.is_valid(Clock::get()?.slot) {
            return Err(BettingPlatformError::RoleExpired.into());
        }
        
        // Check if already assigned
        if self.role_assignments.iter().any(|(u, r)| u == user && *r == role_index) {
            return Err(BettingPlatformError::RoleAlreadyAssigned.into());
        }
        
        // Check member limit
        if role.max_members > 0 && role.member_count >= role.max_members {
            return Err(BettingPlatformError::RoleMemberLimitReached.into());
        }
        
        // Assign role
        self.role_assignments.push((*user, role_index));
        role.member_count += 1;
        self.last_update_slot = Clock::get()?.slot;
        
        msg!("Role {} granted to {}", role_index, user);
        Ok(())
    }
    
    /// Revoke role from user
    pub fn revoke_role(
        &mut self,
        user: &Pubkey,
        role_index: u8,
        authority: &Pubkey,
    ) -> Result<(), ProgramError> {
        // Check authority
        if *authority != self.authority {
            let auth_perms = self.get_user_permissions(authority);
            auth_perms.require(Permissions::MANAGE_ROLES)?;
        }
        
        // Find and remove assignment
        let pos = self.role_assignments.iter()
            .position(|(u, r)| u == user && *r == role_index)
            .ok_or(BettingPlatformError::RoleNotAssigned)?;
        
        self.role_assignments.remove(pos);
        
        // Update role member count
        if let Some(role) = self.roles.get_mut(role_index as usize) {
            role.member_count = role.member_count.saturating_sub(1);
        }
        
        self.last_update_slot = Clock::get()?.slot;
        
        msg!("Role {} revoked from {}", role_index, user);
        Ok(())
    }
    
    /// Grant direct permission
    pub fn grant_permission(
        &mut self,
        user: &Pubkey,
        permission: Permissions,
        authority: &Pubkey,
    ) -> Result<(), ProgramError> {
        // Check authority
        if *authority != self.authority {
            let auth_perms = self.get_user_permissions(authority);
            auth_perms.require(Permissions::MANAGE_ROLES)?;
        }
        
        // Find or create user entry
        if let Some((_, perms)) = self.user_permissions.iter_mut().find(|(u, _)| u == user) {
            perms.add(permission);
        } else {
            if self.user_permissions.len() >= Self::MAX_USERS {
                return Err(BettingPlatformError::UserLimitReached.into());
            }
            self.user_permissions.push((*user, permission));
        }
        
        self.last_update_slot = Clock::get()?.slot;
        
        msg!("Permission {:b} granted to {}", permission.0, user);
        Ok(())
    }
    
    /// Suspend user
    pub fn suspend_user(
        &mut self,
        user: &Pubkey,
        authority: &Pubkey,
    ) -> Result<(), ProgramError> {
        // Check authority
        if *authority != self.authority {
            return Err(ProgramError::MissingRequiredSignature);
        }
        
        if !self.suspended_users.contains(user) {
            self.suspended_users.push(*user);
            self.last_update_slot = Clock::get()?.slot;
            msg!("User {} suspended", user);
        }
        
        Ok(())
    }
    
    /// Unsuspend user
    pub fn unsuspend_user(
        &mut self,
        user: &Pubkey,
        authority: &Pubkey,
    ) -> Result<(), ProgramError> {
        // Check authority
        if *authority != self.authority {
            return Err(ProgramError::MissingRequiredSignature);
        }
        
        if let Some(pos) = self.suspended_users.iter().position(|u| u == user) {
            self.suspended_users.remove(pos);
            self.last_update_slot = Clock::get()?.slot;
            msg!("User {} unsuspended", user);
        }
        
        Ok(())
    }
    
    /// Validate ACL
    pub fn validate(&self) -> Result<(), ProgramError> {
        if self.discriminator != discriminators::ACCESS_CONTROL {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if self.roles.len() > Self::MAX_ROLES {
            return Err(ProgramError::InvalidAccountData);
        }
        
        if self.user_permissions.len() > Self::MAX_USERS {
            return Err(ProgramError::InvalidAccountData);
        }
        
        Ok(())
    }
}

/// Access control context for permission checks
pub struct AccessContext<'a> {
    pub user: &'a Pubkey,
    pub permissions: Permissions,
    pub is_suspended: bool,
}

impl<'a> AccessContext<'a> {
    /// Create from ACL
    pub fn from_acl(acl: &AccessControlList, user: &'a Pubkey) -> Self {
        let permissions = acl.get_user_permissions(user);
        let is_suspended = acl.suspended_users.contains(user);
        
        Self {
            user,
            permissions,
            is_suspended,
        }
    }
    
    /// Require permission
    pub fn require(&self, permission: Permissions) -> Result<(), ProgramError> {
        if self.is_suspended {
            msg!("User {} is suspended", self.user);
            return Err(BettingPlatformError::UserSuspended.into());
        }
        
        self.permissions.require(permission)
    }
    
    /// Check permission (no error)
    pub fn has(&self, permission: Permissions) -> bool {
        !self.is_suspended && self.permissions.has(permission)
    }
}

/// Initialize access control list
pub fn initialize_acl<'a>(
    acl_account: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    payer: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
) -> ProgramResult {
    // Verify account is uninitialized
    if !acl_account.data_is_empty() {
        return Err(ProgramError::AccountAlreadyInitialized);
    }
    
    // Verify authority is signer
    if !authority.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Calculate space needed
    let acl = AccessControlList::new(*authority.key);
    let space = acl.try_to_vec()?.len() + 1000; // Extra space for growth
    
    // Create account
    let rent = solana_program::rent::Rent::get()?;
    let rent_lamports = rent.minimum_balance(space);
    
    solana_program::program::invoke(
        &solana_program::system_instruction::create_account(
            payer.key,
            acl_account.key,
            rent_lamports,
            space as u64,
            &crate::ID,
        ),
        &[payer.clone(), acl_account.clone(), system_program.clone()],
    )?;
    
    // Initialize ACL
    acl.serialize(&mut &mut acl_account.data.borrow_mut()[..])?;
    
    msg!("Access control list initialized with authority {}", authority.key);
    Ok(())
}

/// Macro for requiring permissions
#[macro_export]
macro_rules! require_permission {
    ($acl:expr, $user:expr, $permission:expr) => {{
        let context = $crate::security::AccessContext::from_acl($acl, $user);
        context.require($permission)?;
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permissions() {
        let mut perms = Permissions::NONE;
        assert!(!perms.has(Permissions::CREATE_PROPOSAL));
        
        perms.add(Permissions::CREATE_PROPOSAL);
        assert!(perms.has(Permissions::CREATE_PROPOSAL));
        
        perms.add(Permissions::EXECUTE_TRADES);
        assert!(perms.has(Permissions::CREATE_PROPOSAL));
        assert!(perms.has(Permissions::EXECUTE_TRADES));
        
        perms.remove(Permissions::CREATE_PROPOSAL);
        assert!(!perms.has(Permissions::CREATE_PROPOSAL));
        assert!(perms.has(Permissions::EXECUTE_TRADES));
    }

    #[test]
    fn test_role_management() {
        let authority = Pubkey::new_unique();
        let mut acl = AccessControlList::new(authority);
        let user = Pubkey::new_unique();
        
        // Grant trader role
        assert!(acl.grant_role(&user, 3, &authority).is_ok()); // Index 3 = Trader
        
        // Check permissions
        let perms = acl.get_user_permissions(&user);
        assert!(perms.has(Permissions::CREATE_PROPOSAL));
        assert!(perms.has(Permissions::EXECUTE_TRADES));
        assert!(!perms.has(Permissions::PAUSE_PROTOCOL));
        
        // Revoke role
        assert!(acl.revoke_role(&user, 3, &authority).is_ok());
        let perms = acl.get_user_permissions(&user);
        assert!(!perms.has(Permissions::CREATE_PROPOSAL));
    }

    #[test]
    fn test_suspension() {
        let authority = Pubkey::new_unique();
        let mut acl = AccessControlList::new(authority);
        let user = Pubkey::new_unique();
        
        // Grant admin role
        assert!(acl.grant_role(&user, 0, &authority).is_ok());
        assert!(acl.get_user_permissions(&user).has(Permissions::ADMIN));
        
        // Suspend user
        assert!(acl.suspend_user(&user, &authority).is_ok());
        assert_eq!(acl.get_user_permissions(&user), Permissions::NONE);
        
        // Unsuspend user
        assert!(acl.unsuspend_user(&user, &authority).is_ok());
        assert!(acl.get_user_permissions(&user).has(Permissions::ADMIN));
    }
}