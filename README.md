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

## To Use endpoints

```bash
API=http://localhost:8080
AUTH="Authorization: Bearer dodo_test_live_key_1234567890"
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
