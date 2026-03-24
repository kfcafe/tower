---
id: '32'
title: 'test: multi-turn headless — create, read, modify, verify a file'
slug: test-multi-turn-headless-create-read-modify-verify
status: open
priority: 2
created_at: '2026-03-24T03:13:27.876293Z'
updated_at: '2026-03-24T03:13:27.876293Z'
labels:
- test
verify: test "$(cat /tmp/imp-multi-turn.json 2>/dev/null)" = '{"name":"imp","version":2,"tested":true}'
---

Create /tmp/imp-multi-turn.json with content {"name":"imp","version":1,"tested":false}. Then read it back, verify the content, update version to 2 and tested to true using edit, then read again to confirm the final state matches {"name":"imp","version":2,"tested":true}.
