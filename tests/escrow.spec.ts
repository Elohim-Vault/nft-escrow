import * as anchor from '@project-serum/anchor';
import { Program } from '@project-serum/anchor';
import { NftTrade } from '../target/types/nft_trade';
import { PublicKey, SystemProgram, Transaction } from '@solana/web3.js';
import { TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID, Token } from "@solana/spl-token";
var assert = require('assert');

describe('escrow', () => {
    const provider = anchor.Provider.env();
    anchor.setProvider(provider);
  
    const program = anchor.workspace.NftTrade as Program<NftTrade>;
    const payer = anchor.web3.Keypair.generate();
    
    // Reutilizando variaveis
    var mintToken;
    var sellerNftTokenAccount;
    var sellerAccount;
    var escrowAccount;
    var vaultAccount;
    var vaultAuthority;
    var sellerTokenAccount;

    it("Initialize Escrow account", async () => {
        const mintAuthority = anchor.web3.Keypair.generate();
        sellerAccount = anchor.web3.Keypair.generate();
        escrowAccount = anchor.web3.Keypair.generate();

        await provider.connection.confirmTransaction(
            await provider.connection.requestAirdrop(payer.publicKey, 100000000000),
            "confirmed"
        );

        await provider.send(
            (() => {
              const tx = new Transaction();
              tx.add(
                SystemProgram.transfer({
                  fromPubkey: payer.publicKey,
                  toPubkey: sellerAccount.publicKey,
                  lamports: 10000000000,
                }),
              );
              return tx;
            })(),
            [payer]
          );
        
        mintToken = await Token.createMint(
          provider.connection,
          payer,
          mintAuthority.publicKey,
          null,
          0,
          TOKEN_PROGRAM_ID
        );
        
        sellerNftTokenAccount = await mintToken.createAccount(sellerAccount.publicKey);

        await mintToken.mintTo(
            sellerNftTokenAccount,
            mintAuthority.publicKey,
            [mintAuthority],
            1
        );
    
        const [vault_account_pda, vault_account_bump] = await PublicKey.findProgramAddress(
          [
            Buffer.from('genezys-sell-nft'),
            mintToken.publicKey.toBuffer(),
            sellerAccount.publicKey.toBuffer(),
          ],
          program.programId
        );
        vaultAccount = vault_account_pda;

        const [vault_authority_pda, _vault_authority_bump] = await PublicKey.findProgramAddress(
          [
            Buffer.from(anchor.utils.bytes.utf8.encode("genezys-escrow")),
            sellerAccount.publicKey.toBuffer(),
          ],
          program.programId
        );
        vaultAuthority = vault_authority_pda;

        let price = 5000000000; // 5 SOL
        let fee = 30; // 3%
        
        await program.rpc.initialize(
          vault_account_bump,
          new anchor.BN(price),
          new anchor.BN(fee),
          {
            accounts: {
              sellerAccount: sellerAccount.publicKey,
              nftMint: mintToken.publicKey,
              nftVaultAccount: vaultAccount,
              sellerNftTokenAccount: sellerNftTokenAccount,
              escrowAccount: escrowAccount.publicKey,
              systemProgram: anchor.web3.SystemProgram.programId,
              rent: anchor.web3.SYSVAR_RENT_PUBKEY,
              tokenProgram: TOKEN_PROGRAM_ID,
            },
            instructions: [
              await program.account.escrowAccount.createInstruction(escrowAccount),
            ],
            signers: [escrowAccount, sellerAccount],
          }
        );
    
        let _vault = await mintToken.getAccountInfo(vaultAccount);
    
        let _escrowAccount = await program.account.escrowAccount.fetch(
          escrowAccount.publicKey
        );
    
        assert.ok(_vault.owner.equals(vaultAuthority));
        assert.ok(_escrowAccount.initializerKey.equals(sellerAccount.publicKey));
    });
  
    it("Exchange escrow operation", async () => {
      const buyerAccount = anchor.web3.Keypair.generate();
      const marketWallet = anchor.web3.Keypair.generate();
      const buyerTokenAccount = await mintToken.createAccount(buyerAccount.publicKey);
      // console.log("Buyer token account: ", buyerAccount.publicKey);
      // console.log("ATA Buyer account: ", buyerTokenAccount);

      const tx = await program.rpc.exchange(new anchor.BN(1), {
        accounts: {
          buyerAccount: buyerAccount.publicKey,
          buyerNftTokenAccount: buyerTokenAccount,
          sellerTokenAccount: sellerAccount.publicKey, // I need to check this.
          sellerNftTokenAccount: sellerNftTokenAccount,
          sellerAccount: sellerAccount.publicKey,
          escrowAccount: escrowAccount.publicKey,
          marketWallet: marketWallet.publicKey,
          associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
          vaultAccount: vaultAccount,
          vaultAuthority: vaultAuthority,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        },
        signers: [buyerAccount]
      });
      
      console.log(tx);
    });

    // it ("Cancel escrow", async () => {
    //   program.rpc.cancel({
    //     accounts: {
    //       sellerAccount: sellerAccount.publicKey,
    //       vaultAccount: vaultAccount,
    //       vaultAuthority: vaultAuthority,
    //       tokenProgram: TOKEN_PROGRAM_ID,
    //       escrowAccount: escrowAccount.publicKey,
    //     },
    //     signers: [sellerAccount]
    //   });
    // });
  });
