
use anchor_lang::prelude::*;
use anchor_spl::token::{self, CloseAccount, Mint, SetAuthority, TokenAccount, Transfer};
use spl_token::instruction::AuthorityType;
use anchor_lang::solana_program::{program::invoke, system_instruction};

declare_id!("6yMsjNTLtgkC1rUaJCjPeESt34Cd4nDSyVqn2i4qm1oy");

#[program]
mod nft_trade {
    use super::*;
    pub fn initialize(
        ctx: Context<Initialize>, 
        _valut_account_bump: u8,
        price: u64, // price of nft, if price of nft is 1.5 SOL = 1500000000 lamports
        fee: u8, // fee of nft, if fee is 3.5% = 3500, so decimal is 1
    ) -> ProgramResult {
        if ctx.accounts.escrow_account.is_initialized {
            return Err(ProgramError::AccountAlreadyInitialized);
        }
        if !ctx.accounts.seller_account.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }

        msg!("Log msg");
        ctx.accounts.escrow_account.is_initialized = true;
        ctx.accounts.escrow_account.initializer_key = ctx.accounts.seller_account.key();
        ctx.accounts.escrow_account.seller_nft_token_account = *ctx.accounts.nft_vault_account.to_account_info().key;
        ctx.accounts.escrow_account.seller_amount = 1;
        ctx.accounts.escrow_account.price = price;
        ctx.accounts.escrow_account.fee = fee;

        let (vault_authority, _vault_authority_bump) =
            Pubkey::find_program_address(&[b"genezys-escrow", ctx.accounts.seller_account.key().as_ref()], ctx.program_id);

        // change the authority to program
        
        token::set_authority(
            ctx.accounts.into_set_authority_context(),
            AuthorityType::AccountOwner,
            Some(vault_authority),
        )?;

        // transfer the nft to escrow vault account
        token::transfer(
            ctx.accounts.into_transfer_to_pda_context(),
            ctx.accounts.escrow_account.seller_amount as u64,
        )?;

        Ok(())
    }

    pub fn exchange(
        ctx: Context<Exchange>,
        sol_amount: u64, // SOL amount 1 SOL = 1000000000 lamports
    ) -> ProgramResult {
        if !ctx.accounts.buyer_account.is_signer {
            return Err(ProgramError::MissingRequiredSignature);
        }
        if **ctx.accounts.buyer_account.lamports.borrow() < sol_amount {
            return Err(ProgramError::AccountNotRentExempt);
        }

        if sol_amount != ctx.accounts.escrow_account.price {
            return Err(ProgramError::InvalidAccountData);
        }

        let fee_price = sol_amount * ctx.accounts.escrow_account.fee as u64 / 1000;
        let seller_price = sol_amount - fee_price;

        const ESCROW_PDA_SEED: &[u8] = b"genezys-escrow";

        let (_vault_authority, vault_authority_bump) =
            Pubkey::find_program_address(&[ESCROW_PDA_SEED], ctx.program_id);

        let authority_seeds = &[&ESCROW_PDA_SEED, ctx.accounts.seller_account.key.as_ref(), &[vault_authority_bump]];
        
        // transfer the seller SOL to seller token account
        // token::transfer(
        //     ctx.accounts.into_transfer_to_seller_context(),
        //     seller_price,
        // )?;
        invoke(
            &system_instruction::transfer(
                ctx.accounts.buyer_account.key,
                ctx.accounts.seller_account.key,
                seller_price,
            ),
            &[
                ctx.accounts.buyer_account.clone(),
                ctx.accounts.seller_account.clone(),
                ctx.accounts.system_program.clone(),
            ],
        )?;

        //change the authority to buyer
        token::set_authority(
            ctx.accounts.into_set_authority_to_buyer_context(),
            AuthorityType::AccountOwner,
            Some(*ctx.accounts.buyer_account.key),
        )?;

        // transfer SOL to marketplace wallet
        // token::transfer(
        //     ctx.accounts.into_transfer_to_marker_context(),
        //     fee_price,
        // )?;

        invoke(
            &system_instruction::transfer(
                ctx.accounts.buyer_account.key,
                ctx.accounts.market_wallet.key,
                fee_price,
            ),
            &[
                ctx.accounts.buyer_account.clone(),
                ctx.accounts.market_wallet.clone(),
                ctx.accounts.system_program.clone(),
            ],
        )?;

        // close escrow account
        token::close_account(
            ctx.accounts
                .into_close_context()
                .with_signer(&[&authority_seeds[..]]),
        )?;

        Ok(())
    }

    pub fn cancel(ctx: Context<Cancel>) -> ProgramResult {
        const ESCROW_PDA_SEED: &[u8] = b"genezys-escrow";

        let (_vault_authority, vault_authority_bump) =
            Pubkey::find_program_address(&[ESCROW_PDA_SEED], ctx.program_id);

        let authority_seeds = &[&ESCROW_PDA_SEED, ctx.accounts.seller_account.key.as_ref(), &[vault_authority_bump]];

        //change the authority to seller
        token::set_authority(
            ctx.accounts.into_set_authority_context(),
            AuthorityType::AccountOwner,
            Some(*ctx.accounts.seller_account.key),
        )?;

        //transfer the nft to seller account
        token::transfer(
            ctx.accounts.into_transfer_to_seller_context(),
            ctx.accounts.escrow_account.seller_amount as u64,
        )?;
        
        token::close_account(
            ctx.accounts
                .into_close_context()
                .with_signer(&[&authority_seeds[..]]),
        )?;

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(vault_account_bump: u8)]
pub struct Initialize<'info> {
    // seller nft account
    #[account(mut, signer)]
    pub seller_account: AccountInfo<'info>,
    //nft mint
    pub nft_mint: Box<Account<'info, Mint>>,
    #[account(
        init,
        seeds = [
            b"genezys-sell-nft".as_ref(),
            nft_mint.key().as_ref(),
            seller_account.key().as_ref(),
        ],
        bump = vault_account_bump,
        payer = seller_account,
        token::mint = nft_mint,
        token::authority = seller_account,
    )]
    pub nft_vault_account: Box<Account<'info, TokenAccount>>,
    #[account(
        mut,
        constraint = seller_nft_token_account.amount == 1
    )]
    pub seller_nft_token_account: Box<Account<'info, TokenAccount>>,
    #[account(zero)]
    pub escrow_account: Box<Account<'info, EscrowAccount>>,
    pub system_program: AccountInfo<'info>,
    pub rent: Sysvar<'info, Rent>,
    pub token_program: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct Exchange<'info> {
    // Buyer who buy the NFT
    #[account(signer)]
    pub buyer_account: AccountInfo<'info>,
    // Buyer who receive the NFT on escrow
    #[account(mut)]
    pub buyer_nft_token_account: Box<Account<'info, TokenAccount>>,
    // Seller who receive the SOL on escrow
    #[account(mut)]
    pub seller_token_account: AccountInfo<'info>,
    // Seller who sell the NFT on escrow
    #[account(mut)]
    pub seller_nft_token_account: Box<Account<'info, TokenAccount>>,
    // Seller account
    #[account(mut)]
    pub seller_account: AccountInfo<'info>,
    // Escrow account
    #[account(
        mut,
        constraint = escrow_account.seller_amount == 1,
        constraint = escrow_account.seller_nft_token_account == *seller_nft_token_account.to_account_info().key, // ?!
        constraint = escrow_account.initializer_key == *seller_account.key,
        close = seller_account
    )]
    pub escrow_account: Box<Account<'info, EscrowAccount>>,
    #[account(mut)]
    pub market_wallet: AccountInfo<'info>,
    #[account(mut)]
    pub vault_account: Account<'info, TokenAccount>,
    pub vault_authority: AccountInfo<'info>,
    pub token_program: AccountInfo<'info>,
    pub system_program: AccountInfo<'info>,
    pub associated_token_program: AccountInfo<'info>,
    pub rent: AccountInfo<'info>,
}



#[derive(Accounts)]
pub struct Cancel<'info> {
    #[account(mut, signer)]
    pub seller_account: AccountInfo<'info>,
    #[account(mut)]
    pub vault_account: Account<'info, TokenAccount>,
    pub vault_authority: AccountInfo<'info>,
    
    #[account(
        mut,
        constraint = escrow_account.initializer_key == *seller_account.key,
        close = seller_account
    )]  
    pub escrow_account: Box<Account<'info, EscrowAccount>>,
    pub token_program: AccountInfo<'info>,
}

impl<'info> Initialize<'info> {
    fn into_set_authority_context(&self) -> CpiContext<'_, '_, '_, 'info, SetAuthority<'info>> {
        let cpi_accounts = SetAuthority {
            account_or_mint: self.nft_vault_account.to_account_info().clone(),
            current_authority: self.seller_account.clone(),
        };
        CpiContext::new(self.token_program.clone(), cpi_accounts)
    }

    fn into_transfer_to_pda_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self
                .seller_nft_token_account
                .to_account_info()
                .clone(),
            to: self.nft_vault_account.to_account_info().clone(),
            authority: self.seller_account.clone(),
        };
        CpiContext::new(self.token_program.clone(), cpi_accounts)
    }
}

impl<'info> Exchange<'info> {
    fn _into_transfer_to_seller_context(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.buyer_account.to_account_info().clone(),
            to: self
                .seller_token_account
                .to_account_info()
                .clone(),
            authority: self.buyer_account.clone(),
        };
        CpiContext::new(self.token_program.clone(), cpi_accounts)
    }

    fn into_set_authority_to_buyer_context(&self) -> CpiContext<'_, '_, '_, 'info, SetAuthority<'info>> {
        let cpi_accounts = SetAuthority {
            account_or_mint: self.vault_account.to_account_info().clone(),
            current_authority: self.vault_authority.to_account_info().clone(),
        };
        CpiContext::new(self.token_program.clone(), cpi_accounts)
    }

    fn _into_transfer_to_marker_context(
        &self,
    ) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self.buyer_account.to_account_info().clone(),
            to: self
                .market_wallet
                .to_account_info()
                .clone(),
            authority: self.buyer_account.clone(),
        };
        CpiContext::new(self.token_program.clone(), cpi_accounts)
    }
    
    fn into_close_context(&self) -> CpiContext<'_, '_, '_, 'info, CloseAccount<'info>> {
        let cpi_accounts = CloseAccount {
            account: self.vault_account.to_account_info().clone(),
            destination: self.seller_account.clone(),
            authority: self.vault_authority.clone(),
        };
        CpiContext::new(self.token_program.clone(), cpi_accounts)
    }
}

impl<'info> Cancel<'info> {
    fn into_close_context(&self) -> CpiContext<'_, '_, '_, 'info, CloseAccount<'info>> {
        let cpi_accounts = CloseAccount {
            account: self.vault_account.to_account_info().clone(),
            destination: self.seller_account.clone(),
            authority: self.vault_authority.clone(),
        };
        CpiContext::new(self.token_program.clone(), cpi_accounts)
    }

    fn into_set_authority_context(&self) -> CpiContext<'_, '_, '_, 'info, SetAuthority<'info>> {
        let cpi_accounts = SetAuthority {
            account_or_mint: self.vault_account.to_account_info().clone(),
            current_authority: self.vault_authority.clone(),
        };
        CpiContext::new(self.token_program.clone(), cpi_accounts)
    }

    fn into_transfer_to_seller_context(&self) -> CpiContext<'_, '_, '_, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: self
                .vault_account
                .to_account_info()
                .clone(),
            to: self.seller_account.to_account_info().clone(),
            authority: self.vault_authority.clone(),
        };
        CpiContext::new(self.token_program.clone(), cpi_accounts)
    }
}

#[account]
pub struct EscrowAccount {
    // Check escrow is created or not
    pub is_initialized: bool,
    // initializer key
    pub initializer_key: Pubkey,
    // seller token account: NFT
    pub seller_nft_token_account: Pubkey,
    // seller NFT amount : 1
    pub seller_amount: u8,
    // fee of NFT
    pub fee: u8,
    // price of nft
    pub price: u64
}

