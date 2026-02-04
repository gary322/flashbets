use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program::invoke_signed,
    program_error::ProgramError,
    program_pack::{IsInitialized, Pack},
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
    sysvar::Sysvar,
};
use borsh::{BorshDeserialize, BorshSerialize};

use crate::{
    error::ClassificationError,
    instruction::ClassificationInstruction,
    state::{
        ClassificationEngine, DateFormat, NormalizationConfig, 
        VerseMetadata, VerseRegistry,
    },
    normalization::{
        TextNormalizer, get_default_synonyms, STOPWORDS,
    },
    classification::{
        calculate_verse_id, detect_category, find_similar_verse,
    },
};

pub struct Processor;

impl Processor {
    pub fn process(
        program_id: &Pubkey,
        accounts: &[AccountInfo],
        instruction_data: &[u8],
    ) -> ProgramResult {
        let instruction = ClassificationInstruction::unpack(instruction_data)?;
        
        match instruction {
            ClassificationInstruction::InitializeEngine => {
                msg!("Instruction: InitializeEngine");
                Self::process_initialize_engine(accounts, program_id)
            }
            ClassificationInstruction::ClassifyMarket { market_title, market_id } => {
                msg!("Instruction: ClassifyMarket");
                Self::process_classify_market(accounts, program_id, market_title, market_id)
            }
            ClassificationInstruction::UpdateVerseHierarchy { verse_id, parent_id } => {
                msg!("Instruction: UpdateVerseHierarchy");
                Self::process_update_hierarchy(accounts, program_id, verse_id, parent_id)
            }
            ClassificationInstruction::SearchVerses { keywords, category } => {
                msg!("Instruction: SearchVerses");
                Self::process_search_verses(accounts, program_id, keywords, category)
            }
        }
    }
    
    fn process_initialize_engine(
        accounts: &[AccountInfo],
        program_id: &Pubkey,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let authority_info = next_account_info(account_info_iter)?;
        let engine_info = next_account_info(account_info_iter)?;
        let registry_info = next_account_info(account_info_iter)?;
        let system_program = next_account_info(account_info_iter)?;
        let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;
        
        // Verify authority is signer
        if !authority_info.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        
        // Derive PDA for engine
        let (engine_pubkey, engine_bump) = Pubkey::find_program_address(
            &[b"classification_engine"],
            program_id,
        );
        
        if engine_pubkey != *engine_info.key {
            return Err(ClassificationError::InvalidPDA.into());
        }
        
        // Create engine account
        let engine_size = ClassificationEngine::LEN;
        let engine_lamports = rent.minimum_balance(engine_size);
        
        invoke_signed(
            &system_instruction::create_account(
                authority_info.key,
                engine_info.key,
                engine_lamports,
                engine_size as u64,
                program_id,
            ),
            &[authority_info.clone(), engine_info.clone(), system_program.clone()],
            &[&[b"classification_engine", &[engine_bump]]],
        )?;
        
        // Initialize engine
        let engine = ClassificationEngine::new(*authority_info.key, engine_bump);
        ClassificationEngine::pack(engine, &mut engine_info.try_borrow_mut_data()?)?;
        
        // Derive PDA for registry
        let (registry_pubkey, registry_bump) = Pubkey::find_program_address(
            &[b"verse_registry"],
            program_id,
        );
        
        if registry_pubkey != *registry_info.key {
            return Err(ClassificationError::InvalidPDA.into());
        }
        
        // Create registry account (starting with reasonable size)
        let registry_size = 10_000; // Initial size, can be reallocated later
        let registry_lamports = rent.minimum_balance(registry_size);
        
        invoke_signed(
            &system_instruction::create_account(
                authority_info.key,
                registry_info.key,
                registry_lamports,
                registry_size as u64,
                program_id,
            ),
            &[authority_info.clone(), registry_info.clone(), system_program.clone()],
            &[&[b"verse_registry", &[registry_bump]]],
        )?;
        
        // Initialize registry
        let registry = VerseRegistry::new(registry_bump);
        borsh::to_writer(&mut registry_info.try_borrow_mut_data()?.as_mut(), &registry)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        
        msg!("Classification engine initialized successfully");
        Ok(())
    }
    
    fn process_classify_market(
        accounts: &[AccountInfo],
        program_id: &Pubkey,
        market_title: String,
        market_id: String,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let authority_info = next_account_info(account_info_iter)?;
        let engine_info = next_account_info(account_info_iter)?;
        let registry_info = next_account_info(account_info_iter)?;
        let verse_info = next_account_info(account_info_iter)?;
        let system_program = next_account_info(account_info_iter)?;
        let rent = &Rent::from_account_info(next_account_info(account_info_iter)?)?;
        
        // Load engine
        let mut engine = ClassificationEngine::unpack(&engine_info.try_borrow_data()?)?;
        
        if !engine.is_initialized() {
            return Err(ProgramError::UninitializedAccount);
        }
        
        // Create normalization config from engine settings
        let config = NormalizationConfig {
            lowercase_enabled: engine.lowercase_enabled,
            punctuation_removal: engine.punctuation_removal,
            number_standardization: engine.number_standardization,
            date_format: DateFormat::ISO8601,
            currency_normalization: engine.currency_normalization,
        };
        
        // Get default synonyms
        let synonyms = get_default_synonyms();
        
        // Step 1: Normalize the title
        let normalized = TextNormalizer::normalize_title(&market_title, &config, &synonyms)?;
        msg!("Normalized title: {}", normalized);
        
        // Step 2: Extract keywords
        let keywords = TextNormalizer::extract_keywords(&normalized, &STOPWORDS)?;
        msg!("Keywords: {:?}", keywords);
        
        // Step 3: Calculate verse ID
        let verse_id = calculate_verse_id(&normalized, &keywords)?;
        msg!("Calculated verse ID: {:?}", verse_id);
        
        // Step 4: Load registry and check for similar verses
        let mut registry_data = registry_info.try_borrow_mut_data()?;
        let mut registry: VerseRegistry = VerseRegistry::try_from_slice(&registry_data)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        
        let similar_verse = find_similar_verse(
            &registry,
            &normalized,
            &keywords,
            engine.similarity_threshold,
        )?;
        
        if let Some(existing_id) = similar_verse {
            msg!("Found similar verse: {:?}", existing_id);
            // Update existing verse (would need to load and update the verse account)
            Ok(())
        } else {
            // Create new verse
            msg!("Creating new verse");
            
            // Derive PDA for verse
            let (verse_pubkey, verse_bump) = Pubkey::find_program_address(
                &[b"verse", &verse_id],
                program_id,
            );
            
            if verse_pubkey != *verse_info.key {
                return Err(ClassificationError::InvalidPDA.into());
            }
            
            // Detect category
            let category = detect_category(&normalized, &keywords)?;
            msg!("Detected category: {}", category);
            
            // Calculate account size
            let verse_size = VerseMetadata::calculate_len(
                market_title.len(),
                normalized.len(),
                &keywords,
                category.len(),
                0, // No children initially
            );
            
            // Create verse account
            let verse_lamports = rent.minimum_balance(verse_size);
            
            invoke_signed(
                &system_instruction::create_account(
                    authority_info.key,
                    verse_info.key,
                    verse_lamports,
                    verse_size as u64,
                    program_id,
                ),
                &[authority_info.clone(), verse_info.clone(), system_program.clone()],
                &[&[b"verse", &verse_id, &[verse_bump]]],
            )?;
            
            // Initialize verse metadata
            let verse_metadata = VerseMetadata::new(
                verse_id,
                market_title,
                normalized,
                keywords.clone(),
                category.clone(),
                verse_bump,
            );
            
            borsh::to_writer(&mut verse_info.try_borrow_mut_data()?.as_mut(), &verse_metadata)
                .map_err(|_| ProgramError::InvalidAccountData)?;
            
            // Update registry
            for keyword in &keywords {
                registry.add_verse_to_keyword(keyword, verse_id)?;
            }
            registry.add_verse_to_category(&category, verse_id)?;
            registry.total_verses += 1;
            
            // Save registry
            borsh::to_writer(&mut registry_data.as_mut(), &registry)
                .map_err(|_| ProgramError::InvalidAccountData)?;
            
            // Update engine
            engine.total_verses += 1;
            engine.total_markets += 1;
            ClassificationEngine::pack(engine, &mut engine_info.try_borrow_mut_data()?)?;
            
            msg!("New verse created successfully");
            Ok(())
        }
    }
    
    fn process_update_hierarchy(
        accounts: &[AccountInfo],
        program_id: &Pubkey,
        verse_id: [u8; 16],
        parent_id: Option<[u8; 16]>,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let authority_info = next_account_info(account_info_iter)?;
        let engine_info = next_account_info(account_info_iter)?;
        let verse_info = next_account_info(account_info_iter)?;
        let parent_verse_info = next_account_info(account_info_iter)?;
        
        // Verify authority
        let engine = ClassificationEngine::unpack(&engine_info.try_borrow_data()?)?;
        if *authority_info.key != engine.authority {
            return Err(ClassificationError::Unauthorized.into());
        }
        
        // Load verse metadata
        let mut verse_data = verse_info.try_borrow_mut_data()?;
        let mut verse: VerseMetadata = VerseMetadata::try_from_slice(&verse_data)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        
        // Update parent relationship
        if let Some(pid) = parent_id {
            // Verify parent exists and update it
            let mut parent_data = parent_verse_info.try_borrow_mut_data()?;
            let mut parent: VerseMetadata = VerseMetadata::try_from_slice(&parent_data)
                .map_err(|_| ProgramError::InvalidAccountData)?;
            
            // Check depth limit
            let mut depth = 1u8;
            let mut current_parent = parent.parent_verse;
            while let Some(_) = current_parent {
                depth += 1;
                if depth >= engine.max_verse_depth {
                    return Err(ClassificationError::MaxDepthExceeded.into());
                }
                // In real implementation, would need to load parent accounts
                current_parent = None; // Break for now
            }
            
            // Update relationships
            verse.set_parent(pid);
            parent.add_child(verse_id)?;
            
            // Save both
            borsh::to_writer(&mut verse_data.as_mut(), &verse)
                .map_err(|_| ProgramError::InvalidAccountData)?;
            borsh::to_writer(&mut parent_data.as_mut(), &parent)
                .map_err(|_| ProgramError::InvalidAccountData)?;
        }
        
        msg!("Verse hierarchy updated successfully");
        Ok(())
    }
    
    fn process_search_verses(
        accounts: &[AccountInfo],
        program_id: &Pubkey,
        keywords: Vec<String>,
        category: Option<String>,
    ) -> ProgramResult {
        let account_info_iter = &mut accounts.iter();
        let registry_info = next_account_info(account_info_iter)?;
        
        // Load registry
        let registry_data = registry_info.try_borrow_data()?;
        let registry: VerseRegistry = VerseRegistry::try_from_slice(&registry_data)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        
        // Search by keywords
        let mut results = Vec::new();
        for keyword in &keywords {
            let verses = registry.find_verses_by_keyword(keyword);
            results.extend(verses);
        }
        
        // Filter by category if provided
        if let Some(cat) = category {
            let category_verses = registry.find_verses_by_category(&cat);
            results.retain(|v| category_verses.contains(v));
        }
        
        // Remove duplicates
        results.sort();
        results.dedup();
        
        msg!("Found {} matching verses", results.len());
        for (i, verse_id) in results.iter().enumerate().take(10) {
            msg!("Result {}: {:?}", i, verse_id);
        }
        
        Ok(())
    }
}