use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

use crate::{offer::offer_state::OfferState, constants::{SEED_OFFER, SEED_MAIN_STATE}, _main::main_state::MainState, utils::tranfer_token, error::MyError, events};

pub fn accept_offer(ctx: Context<AAcceptOffer>, amount: u64) -> Result<()> {
    let acceptor = ctx.accounts.acceptor.to_account_info();
    let acceptor_offered_token_ata = ctx.accounts.acceptor_offered_token_ata.to_account_info();
    let acceptor_requested_token_ata = ctx.accounts.acceptor_requested_token_ata.to_account_info();
    let offeror_requested_token_ata = ctx.accounts.offeror_requested_token_ata.to_account_info();
    let fee_receiver_ata = ctx.accounts.fee_receiver_ata.to_account_info();
    let main_state = & ctx.accounts.main_state_account;
    let offer_state= &mut ctx.accounts.offer_state_account;
    let offer_state_ata= ctx.accounts.offer_state_account_ata.to_account_info();
    let token_program= ctx.accounts.token_program.to_account_info();

    if !offer_state.is_active{
        return anchor_lang::err!(MyError::OfferNotActive);
    }

    if amount < offer_state.min_offered_amount{
        return anchor_lang::err!(MyError::TooLowAmount);
    }
    let requested_amount = (amount  as f64 * offer_state.ratio) as u64;
    
    //NOTE: Transfering the fees
    let fees = (main_state.fee_rate * requested_amount as f64) as u64;
    msg!("Fees : {}",fees);
    tranfer_token(
        acceptor_requested_token_ata.to_account_info(), 
        fee_receiver_ata, 
        acceptor.to_account_info(), 
        token_program.to_account_info(), 
        fees
    ).map_err(|_|MyError::NotEnoughToken)?;

    //NOTE: Transfering the requested token to offeror ata
    tranfer_token(
        acceptor_requested_token_ata.to_account_info(), 
        offeror_requested_token_ata, 
        acceptor.to_account_info(), 
        token_program.to_account_info(), 
        requested_amount, 
    ).map_err(|_|MyError::NotEnoughToken)?;

    //NOTE: Transfering the offered token from contract accounts.
    let (_, bump) = Pubkey::find_program_address(
        &[
            SEED_OFFER, 
            offer_state.offeror.key().as_ref(), 
            ctx.accounts.offered_token.key().as_ref(),
            ctx.accounts.requested_token.key().as_ref(),
        ], 
        ctx.program_id
    );
    
    let cpi_accounts = Transfer{
        authority: offer_state.to_account_info(),
        from: offer_state_ata,
        to: acceptor_offered_token_ata,
    };

    token::transfer(CpiContext::new_with_signer(
        token_program, 
        cpi_accounts, 
        &[
            &[
                SEED_OFFER, 
                offer_state.offeror.key().as_ref(), 
                ctx.accounts.offered_token.key().as_ref(),
                ctx.accounts.requested_token.key().as_ref(),
                &[bump]
            ]
        ]
    ), amount)?;

    //NOTE: set the state
    offer_state.offered_amount -= amount;
    offer_state.requested_amount -= requested_amount;

    if offer_state.min_offered_amount > offer_state.offered_amount{
        offer_state.min_offered_amount = offer_state.offered_amount;
    }

    emit!(events::OfferAccepted{
        offer_id: offer_state.key(),
        amount: amount,
    });

    if offer_state.offered_amount == 0{
        offer_state.re_init();

        emit!(events::OfferCompleted{
            offer_id: offer_state.key(),
        });
    }


    Ok(())
}

#[derive(Accounts)]
pub struct AAcceptOffer<'info> {
    pub acceptor: Signer<'info>,

    ///CHECK:
    pub offered_token: AccountInfo<'info>,
    ///CHECK:
    pub requested_token: AccountInfo<'info>,

    #[account(
        seeds = [SEED_MAIN_STATE],
        bump,
    )]
    pub main_state_account: Account<'info, MainState>,

    ///CHECK:
    #[account(
        mut,
        // token::mint = offer_state_account.offered_token,
        // token::authority = acceptor
    )]
    // pub acceptor_offered_token_ata: Account<'info, TokenAccount>,
    pub acceptor_offered_token_ata: AccountInfo<'info>,

    ///CHECK:
    #[account(
        mut,
        // token::mint = offer_state_account.requested_token,
        // token::authority = acceptor
    )]
    // pub acceptor_requested_token_ata: Account<'info, TokenAccount>,
    pub acceptor_requested_token_ata: AccountInfo<'info>,
    
    #[account(
        mut,
        token::mint = offer_state_account.requested_token,
        token::authority =  offer_state_account.offeror,
    )]
    pub offeror_requested_token_ata: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [
            SEED_OFFER, 
            // offeror_requested_token_ata.owner.as_ref(), 
            offer_state_account.offeror.as_ref(),
            offered_token.key().as_ref(),
            requested_token.key().as_ref(),
        ],
        bump,
    )]
    pub offer_state_account: Account<'info, OfferState>,

    #[account(
        mut,
        token::mint = offered_token,
        token::authority = offer_state_account,
    )]
    pub offer_state_account_ata: Account<'info, TokenAccount>,

    #[account(
        mut,
        token::mint = requested_token,
        token::authority = main_state_account.fee_receiver,
    )]
    pub fee_receiver_ata: Account<'info ,TokenAccount>,

    pub token_program: Program<'info, Token>,
}