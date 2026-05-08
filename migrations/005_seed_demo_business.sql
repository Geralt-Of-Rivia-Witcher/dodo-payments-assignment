-- Local dev seed data only. Kept deterministic for repeatable testing.

INSERT INTO businesses (id, name)
VALUES ('11111111-1111-1111-1111-111111111111', 'Demo Business')
ON CONFLICT (id) DO NOTHING;

-- Plaintext local key (for docs/testing later): dodo_test_live_key_1234567890
-- We store only its hash in DB.
INSERT INTO api_keys (id, business_id, key_prefix, key_hash)
VALUES (
  '22222222-2222-2222-2222-222222222222',
  '11111111-1111-1111-1111-111111111111',
  'dodo_test',
  'c6ee5ed58311a1b613ca304e165c99a7b4f79f94e37f3a3b21ff797ca6add075'
)
ON CONFLICT (id) DO NOTHING;
