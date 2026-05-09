# Dodo Backend Hiring Assignment (Rust)

Minimal Invoice & Payment Service with mock PSP and webhook delivery worker.

## Tech

- Rust + Axum
- PostgreSQL
- SQLx migrations
- Docker Compose

## Run

```bash
docker compose up --build
```

Services:

- Invoice API: `http://localhost:8080`
- Mock PSP: `http://localhost:8081`
- Postgres: `localhost:5432`

Notes:

- Migrations run automatically on Invoice API startup.
- Seed API key (local): `dodo_test_live_key_1234567890`

## Required Curl Examples

Set base values:

```bash
API=http://localhost:8080
AUTH="Authorization: Bearer dodo_test_live_key_1234567890"
```

### 1) Create Customer

```bash
curl -s -X POST "$API/customers" \
  -H "$AUTH" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Alice",
    "email": "alice@example.com"
  }'
```

### 2) Create Invoice

Replace `CUSTOMER_ID` from previous response.

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

### 3) Attempt Payment (Success)

Replace `INVOICE_ID` from previous response.

```bash
curl -s -X POST "$API/invoices/INVOICE_ID/pay" \
  -H "$AUTH" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: pay-success-1" \
  -d '{
    "card_token": "tok_success"
  }'
```

### 4) Attempt Payment (Failure)

Use a different invoice in `open` state (or recreate one), then:

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

Run from workspace root:

```bash
cargo test
```

Included required tests:

- Concurrency test for concurrent `/pay` on same invoice
- Idempotency retry test (same key, same request)
- PSP failure test (`tok_timeout` path)

## Demo Video

Video link:

- `https://drive.google.com/file/d/1MybeL9hkMUsYTal5gTsP-MUZMPtvW0gh/view?usp=sharing`
