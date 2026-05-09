# Dodo Backend Hiring Assignment (Rust)

## Run

Create file: `services/invoice-api/.env`

```env
DATABASE_URL=postgresql://postgres:postgres@localhost:5432/dodo_invoice
PSP_BASE_URL=http://localhost:8081
```

Then run:

```bash
docker compose up --build
```

## Required Curl Examples

```bash
API=http://localhost:8080
AUTH="Authorization: Bearer dodo_test_live_key_1234567890"
```

### 1) Create customer

```bash
curl -s -X POST "$API/customers" \
  -H "$AUTH" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Alice",
    "email": "alice@example.com"
  }'
```

### 2) Create invoice

```bash
curl -s -X POST "$API/invoices" \
  -H "$AUTH" \
  -H "Content-Type: application/json" \
  -d '{
    "customer_id": "CUSTOMER_ID",
    "due_date": "2026-06-10",
    "line_items": [
      {
        "description": "Plan A",
        "quantity": 2,
        "unit_amount_cents": 500
      }
    ]
  }'
```

### 3) Attempt payment (success)

```bash
curl -s -X POST "$API/invoices/INVOICE_ID/pay" \
  -H "$AUTH" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: pay-success-1" \
  -d '{
    "card_token": "tok_success"
  }'
```

### 4) Attempt payment (failure)

```bash
curl -s -X POST "$API/invoices/INVOICE_ID/pay" \
  -H "$AUTH" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: pay-fail-1" \
  -d '{
    "card_token": "tok_card_declined"
  }'
```

## API Documentation
- See [API.md](./API.md)

## Design Document
- See [DESIGN.md](./DESIGN.md)

## AI Usage
- See [AI_USAGE.md](./AI_USAGE.md)

## Tests

```bash
cargo test
```

## Demo Video
- https://drive.google.com/file/d/1MybeL9hkMUsYTal5gTsP-MUZMPtvW0gh/view?usp=sharing
