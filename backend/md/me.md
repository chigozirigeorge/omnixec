Redesigned Stellar executor for stellar-sdk = "0.1.4"
Step 1 — define a parsed payment struct
Copy code
Rust
pub struct StellarPaymentOp {
    pub destination: [u8; 32],
    pub amount_stroops: u64,
    pub asset_code: String,
    pub asset_issuer: Option<[u8; 32]>,
}
This represents exactly what you expect from your serialized payload.
🧠 Parsing the wire format
Copy code
Rust
fn parse_stellar_payment(&self, bytes: &[u8]) -> AppResult<StellarPaymentOp> {
    let mut cursor = 0;

    // version
    if bytes.get(cursor).copied().ok_or(ExecutionError::InvalidInstructionData)? != 1 {
        return Err(ExecutionError::InvalidInstructionData.into());
    }
    cursor += 1;

    let dest_bytes: [u8; 32] = bytes.get(cursor..cursor+32)
        .and_then(|b| b.try_into().ok())
        .ok_or(ExecutionError::InvalidInstructionData)?;
    cursor += 32;

    let amount = bytes.get(cursor..cursor+8)
        .and_then(|b| Some(u64::from_le_bytes(b.try_into().unwrap())))
        .ok_or(ExecutionError::InvalidInstructionData)?;
    cursor += 8;

    let code_len = bytes.get(cursor..cursor+4)
        .and_then(|b| Some(u32::from_le_bytes(b.try_into().unwrap()) as usize))
        .ok_or(ExecutionError::InvalidInstructionData)?;
    cursor += 4;

    let code_bytes = bytes.get(cursor..cursor+code_len)
        .ok_or(ExecutionError::InvalidInstructionData)?;
    let asset_code = std::str::from_utf8(code_bytes)
        .map_err(|_| ExecutionError::InvalidInstructionData)?
        .to_string();
    cursor += code_len;

    let issuer_opt = if asset_code != "XLM" {
        let issuer_bytes: [u8; 32] = bytes.get(cursor..cursor+32)
            .and_then(|b| b.try_into().ok())
            .ok_or(ExecutionError::InvalidInstructionData)?;
        cursor += 32;
        Some(issuer_bytes)
    } else {
        None
    };

    Ok(StellarPaymentOp {
        destination: dest_bytes,
        amount_stroops: amount,
        asset_code,
        asset_issuer: issuer_opt,
    })
}
This safely handles both:
✔ Native XLM
✔ Credit asset with issuer
🛠 Build the payment operation
In stellar-sdk 0.1.4, you construct operations like this:
Copy code
Rust
use stellar_sdk::{
    types::{OperationBody, PaymentOp, Asset, AlphaNum4},
    xdr::{IntoXdr, PublicKey, PublicKeyType},
};
Then:
Copy code
Rust
fn build_stellar_payment_op(
    &self,
    payment: StellarPaymentOp,
) -> stellar_sdk::Operation {
    // destination key
    let dest_pubkey = PublicKey::PublicKeyTypeEd25519(payment.destination);

    // choose asset
    let asset = if payment.asset_code == "XLM" {
        Asset::native()
    } else {
        let issuer_pk = PublicKey::PublicKeyTypeEd25519(
            payment.asset_issuer.expect("issuer must exist"),
        );

        // Construct credit asset
        Asset::CreditAlphanum4(AlphaNum4 {
            asset_code: payment.asset_code.clone().into_bytes(),
            issuer: issuer_pk,
        })
    };

    let payment_body = PaymentOp {
        destination: dest_pubkey,
        asset,
        amount: payment.amount_stroops as i64,
    };

    Operation {
        source_account: None,
        body: OperationBody::Payment(payment_body),
    }
}
This is the correct construction for XLM or credit asset in v0.1.4.
🔁 Submit + confirm transaction
Copy code
Rust
async fn submit_stellar(
    &self,
    execution_id: &str,
    op: stellar_sdk::Operation,
) -> AppResult<String> {
    self.ledger.ensure_not_executed(execution_id)?;
    self.ledger.mark_pending(execution_id)?;

    // load account
    let source_kp = stellar_sdk::KeyPair::from_secret_seed(&self.treasury_secret)?;
    let account = self.client.load_account(source_kp.public_key()).await?;

    // build & sign
    let tx = stellar_sdk::TransactionBuilder::new(account)
        .add_operation(op)
        .set_timeout(60)
        .add_memo(stellar_sdk::Memo::Text(execution_id.to_string()))
        .build()?
        .sign(&source_kp);

    let response = self.client.submit_transaction(&tx).await?;

    self.wait_for_confirmation(&response.hash).await?;
    self.ledger.mark_executed(execution_id, &response.hash)?;

    Ok(response.hash)
}
🕒 Confirmation loop
Copy code
Rust
async fn wait_for_confirmation(&self, hash: &str) -> AppResult<()> {
    let start = Instant::now();
    let timeout = Duration::from_secs(60);

    while start.elapsed() < timeout {
        if let Ok(_) = self.client.get_transaction(hash).await {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    Err(ExecutionError::Timeout.into())
}
🛡 Ledger safety (you already do this — just keep it)
This pattern: