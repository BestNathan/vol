> ## Documentation Index
> Fetch the complete documentation index at: https://docs.deribit.com/llms.txt
> Use this file to discover all available pages before exploring further.

# Test Request(1)

The Test `Request`(`1`) message forces a heartbeat from the opposing
application. The opposing application responds with a
[`Heartbeat`(`0`)](/fix-api/production/heartbeat) containing the `TestReqID`(`112`).

### Arguments

| Tag | Name        | Type   | Required | Comments                        |
| --- | ----------- | ------ | -------- | ------------------------------- |
| 112 | `TestReqId` | String | Yes      | Mirrors the original request ID |
