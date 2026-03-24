---
id: '31'
title: 'test: create a file /tmp/imp-headless-test.txt with ''headless works'''
slug: test-create-a-file-tmpimp-headless-testtxt-with-he
status: closed
priority: 2
created_at: '2026-03-24T02:40:08.572169Z'
updated_at: '2026-03-24T07:11:39.881858Z'
labels:
- test
closed_at: '2026-03-24T07:11:39.881858Z'
close_reason: verify passed (tidy sweep)
verify: cat /tmp/imp-headless-test.txt 2>/dev/null | grep -q 'headless works'
is_archived: true
history:
- attempt: 1
  started_at: '2026-03-24T07:11:26.271886Z'
  finished_at: '2026-03-24T07:11:30.645700Z'
  duration_secs: 4.373
  result: pass
  exit_code: 0
---

Simple test unit: write 'headless works' to /tmp/imp-headless-test.txt using the write tool.
