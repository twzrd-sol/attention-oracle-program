# X402 Payment Integration Guide

This guide explains how to integrate x402 payments into the MILO protocol using the bearer-claims program.

## Overview

X402 is a payment protocol that brings the HTTP 402 "Payment Required" status code to life on Solana. It enables AI agents, APIs, and services to require micropayments before serving content. The MILO protocol now supports x402 payments through the bearer-claims program.

## Architecture

The x402 payment integration consists of:

1. **Payment Creation**: Create payment requests tied to bearer tokens
2. **Payment Processing**: Process payments with transaction signatures
3. **Payment Refunds**: Refund payments when necessary
4. **Status Tracking**: Track payment status throughout the lifecycle

## Key Components

### Payment Account Structure

```rust
pub struct X402Payment {
    pub payment_id: Pubkey,
    pub payer: Pubkey,
    pub recipient: Pubkey,
    pub amount: u64,
    pub bearer_token: Pubkey,
    pub status: PaymentStatus,
    pub created_at: i64,
    pub expires_at: i64,
    pub processed_at: Option<i64>,
    pub transaction_signature: Option<[u8; 64]>,
    pub payment_metadata: Vec<u8>,
    pub bump: u8,
}
```

### Payment Status States

- `Pending`: Payment created but not yet processed
- `Processed`: Payment successfully processed
- `Refunded`: Payment has been refunded
- `Expired`: Payment deadline passed without processing
- `Failed`: Payment processing failed

## Usage Examples

### 1. Creating an X402 Payment

```typescript
import { X402PaymentClient } from "./sdk/x402";

const client = new X402PaymentClient(programId, connection);

const paymentRequest = {
  paymentId: X402PaymentClient.generatePaymentId(),
  amount: 1000000, // 0.001 SOL in lamports
  expiresAt: Math.floor(Date.now() / 1000) + 3600, // 1 hour
  recipient: new PublicKey("RECIPIENT_PUBKEY"),
  metadata: new TextEncoder().encode("API access payment"),
};

const transaction = await client.createPayment(
  paymentRequest,
  payerKeypair,
  bearerTokenPubkey
);

// Sign and send transaction
const signature = await connection.sendTransaction(
  transaction,
  [payerKeypair]
);
```

### 2. Processing a Payment

```typescript
const transactionSignature = new Uint8Array(64); // From actual transaction

const processTx = await client.processPayment(
  paymentAccount,
  transactionSignature,
  processorKeypair
);

const signature = await connection.sendTransaction(
  processTx,
  [processorKeypair]
);
```

### 3. Checking Payment Status

```typescript
const status = await client.getPaymentStatus(paymentAccount);
console.log("Payment status:", status.status);
console.log("Created at:", new Date(status.createdAt * 1000));
if (status.processedAt) {
  console.log("Processed at:", new Date(status.processedAt * 1000));
}
```

### 4. HTTP 402 Response Implementation

```typescript
import express from "express";
import { X402PaymentClient } from "./sdk/x402";

const app = express();
const x402Client = new X402PaymentClient(programId);

app.get("/protected-endpoint", async (req, res) => {
  // Check for payment header
  const paymentHeader = req.headers["x-payment"];
  
  if (!paymentHeader) {
    // Return 402 with payment requirements
    const paymentRequest = {
      paymentId: X402PaymentClient.generatePaymentId(),
      amount: 1000000,
      expiresAt: Math.floor(Date.now() / 1000) + 3600,
      recipient: new PublicKey("YOUR_RECIPIENT_PUBKEY"),
    };
    
    const response = X402PaymentClient.create402Response(
      paymentRequest,
      programId
    );
    
    return res.status(response.status)
      .set(response.headers)
      .json(response.body);
  }
  
  // Verify payment
  const isValid = await x402Client.verifyPayment(paymentHeader as string);
  
  if (!isValid) {
    return res.status(402).json({
      error: "Invalid or expired payment",
      message: "Please provide a valid payment",
    });
  }
  
  // Payment valid, serve protected content
  res.json({ data: "Protected content accessible" });
});
```

## Integration with Existing Bearer Tokens

X402 payments are tightly integrated with the existing bearer token system:

1. **Bearer Token Requirement**: Each x402 payment must be associated with an active bearer token
2. **Owner Validation**: Only the bearer token owner can create payments
3. **Status Inheritance**: Bearer token status affects payment creation

```typescript
// Create bearer token first
const bearerTx = await program.methods
  .createBearerToken(
    streamerKey,
    channel,
    new BN(epoch),
    index,
    new BN(amount),
    rootHash
  )
  .accounts({
    bearerToken: bearerTokenPubkey,
    owner: payer.publicKey,
    // ... other accounts
  })
  .rpc();

// Then create x402 payment associated with the bearer token
const paymentTx = await program.methods
  .createX402Payment(
    paymentId,
    new BN(paymentAmount),
    new BN(expiresAt),
    metadata
  )
  .accounts({
    payer: payer.publicKey,
    recipient: recipientPubkey,
    payment: paymentAccount,
    bearerToken: bearerTokenPubkey, // Link to bearer token
    systemProgram: SystemProgram.programId,
  })
  .rpc();
```

## Error Handling

The x402 payment system includes comprehensive error handling:

```typescript
try {
  await client.createPayment(paymentRequest, payer, bearerToken);
} catch (error) {
  if (error.toString().includes("InvalidAmount")) {
    console.error("Payment amount must be greater than 0");
  } else if (error.toString().includes("InvalidExpiration")) {
    console.error("Expiration time must be in the future");
  } else if (error.toString().includes("NotOwner")) {
    console.error("Only bearer token owner can create payments");
  } else {
    console.error("Payment creation failed:", error);
  }
}
```

## Best Practices

### 1. Payment Expiration
- Set reasonable expiration times (1-24 hours)
- Consider cleanup for expired payments
- Implement retry mechanisms for failed payments

### 2. Amount Validation
- Validate amounts before creating payments
- Use lamports for precision
- Consider minimum payment thresholds

### 3. Security Considerations
- Verify payment signatures before processing
- Implement rate limiting for payment creation
- Monitor for suspicious payment patterns

### 4. Monitoring and Analytics
- Track payment success rates
- Monitor payment processing times
- Analyze payment patterns and volumes

## Testing

Run the x402 payment tests:

```bash
anchor test --skip-deploy
```

The test suite covers:
- Payment creation and validation
- Payment processing and status updates
- Payment refunds
- Error handling for invalid requests
- Expiration handling

## Deployment

### 1. Program Deployment
Deploy the updated bearer-claims program with x402 support:

```bash
anchor deploy
```

### 2. Client Integration
Update your client applications to use the x402 SDK:

```typescript
import { X402PaymentClient } from "@milo/bearer-sdk";

const client = new X402PaymentClient(
  new PublicKey("YOUR_PROGRAM_ID"),
  new Connection("https://api.devnet.solana.com")
);
```

### 3. API Integration
Implement HTTP 402 responses in your APIs:

```typescript
const response = X402PaymentClient.create402Response(
  paymentRequest,
  programId
);
```

## Troubleshooting

### Common Issues

1. **"InvalidAmount" Error**
   - Ensure payment amount is greater than 0
   - Use lamports for amount specification

2. **"InvalidExpiration" Error**
   - Set expiration time in the future
   - Use Unix timestamp in seconds

3. **"NotOwner" Error**
   - Ensure the payer is the bearer token owner
   - Check bearer token status is Active

4. **"PaymentExpired" Error**
   - Process payments before expiration
   - Implement payment retry logic

### Debug Tools

Use the Solana explorer to track payment transactions:
- Devnet: https://explorer.solana.com/?cluster=devnet
- Mainnet: https://explorer.solana.com/

## Support

For support with x402 integration:
- Check the [MILO Protocol documentation](https://docs.milo.xyz)
- Review the [x402 specification](https://solana.com/x402)
- Join the community Discord for assistance
