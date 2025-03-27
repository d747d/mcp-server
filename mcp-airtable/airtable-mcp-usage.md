# Airtable MCP Server Usage Guide

## Overview

This document provides guidance on how to use the Airtable MCP (Model-Controller-Provider) server, which serves as a comprehensive integration layer between your application and the Airtable API. The server implements best practices for Airtable API interaction, including:

- Feature differentiation based on billing plans
- Safety mechanisms for destructive operations
- Comprehensive error handling
- Rate limiting in accordance with Airtable's guidelines
- Token scope validation

## Authentication

All requests to the MCP server must be authenticated with an Airtable token in the `Authorization` header:

```
Authorization: Bearer YOUR_AIRTABLE_TOKEN
```

### Token Types

- **Personal Access Tokens**: For personal development and integrations you build for yourself or your organization
- **OAuth Access Tokens**: For public integrations where other users grant your service access to Airtable

## Billing Plan Features

The MCP server enforces feature availability based on the user's Airtable billing plan:

| Feature | Free | Teams | Business | Enterprise Scale |
|---------|------|-------|----------|------------------|
| Base Data (records, CRUD) | ✓ | ✓ | ✓ | ✓ |
| Views | ✗ | ✗ | ✗ | ✓ |
| Schema Modification | ✗ | ✗ | ✗ | ✓ |
| Webhooks | ✗ | ✗ | ✗ | ✓ |
| SCIM User Management | ✗ | ✗ | ✓ | ✓ |
| Enterprise API | ✗ | ✗ | ✗ | ✓ |
| Change Events | ✗ | ✗ | ✗ | ✓ |

For testing, you can simulate different billing plans by setting the following header:

```
X-Billing-Plan: free|teams|business|enterprise_scale
```

## Safety Mechanisms for Destructive Operations

The MCP server implements safety mechanisms for destructive operations:

### Destructive Updates (PUT)

When using a destructive update (PUT), which clears all unincluded cell values, you must include a confirmation header:

```
X-Confirm-Destructive-Operation: true
```

### Record Deletion

When deleting records, you must include the same confirmation header:

```
X-Confirm-Destructive-Operation: true
```

## Rate Limiting

The MCP server enforces rate limits in accordance with Airtable's guidelines:

- 5 requests per second per base
- 50 requests per second for all traffic using personal access tokens from a given user

When rate limited, the server returns a 429 status code and requires a 30-second waiting period.

## API Endpoints

### Bases

- `GET /api/bases`: List all accessible bases
- `GET /api/bases/:baseId/schema`: Get schema for a specific base

### Records

- `GET /api/bases/:baseId/tables/:tableIdOrName/records`: List records in a table
- `GET /api/bases/:baseId/tables/:tableIdOrName/records/:recordId`: Get a specific record
- `POST /api/bases/:baseId/tables/:tableIdOrName/records`: Create records
- `PATCH /api/bases/:baseId/tables/:tableIdOrName/records`: Update records (partial update)
- `DELETE /api/bases/:baseId/tables/:tableIdOrName/records?records=id1,id2,...`: Delete records

### Schema Modification (Enterprise Scale Plan Only)

- `POST /api/bases/:baseId/tables/:tableId/fields`: Create a new field
- `PATCH /api/bases/:baseId/tables/:tableId/fields/:fieldId`: Update a field

### Webhooks (Enterprise Scale Plan Only)

- `GET /api/bases/:baseId/webhooks`: List webhooks for a base
- `POST /api/bases/:baseId/webhooks`: Create a webhook
- `DELETE /api/bases/:baseId/webhooks/:webhookId`: Delete a webhook

### Enterprise Features (Enterprise Scale Plan Only)

- `GET /api/enterprise/audit-logs`: Get audit logs
- `GET /api/enterprise/users`: Get enterprise users

## Example Usage

### List Records

```javascript
// List records in a table
fetch('http://localhost:3000/api/bases/appXXXXXXXXXXXXXX/tables/tblXXXXXXXXXXXXXX/records', {
  method: 'GET',
  headers: {
    'Authorization': 'Bearer YOUR_AIRTABLE_TOKEN'
  }
})
.then(response => response.json())
.then(data => console.log(data))
.catch(error => console.error('Error:', error));
```

### Create Records

```javascript
// Create new records
fetch('http://localhost:3000/api/bases/appXXXXXXXXXXXXXX/tables/tblXXXXXXXXXXXXXX/records', {
  method: 'POST',
  headers: {
    'Authorization': 'Bearer YOUR_AIRTABLE_TOKEN',
    'Content-Type': 'application/json'
  },
  body: JSON.stringify({
    records: [
      {
        fields: {
          Name: 'John Doe',
          Email: 'john@example.com'
        }
      },
      {
        fields: {
          Name: 'Jane Doe',
          Email: 'jane@example.com'
        }
      }
    ]
  })
})
.then(response => response.json())
.then(data => console.log(data))
.catch(error => console.error('Error:', error));
```

### Update Records

```javascript
// Update records (partial update)
fetch('http://localhost:3000/api/bases/appXXXXXXXXXXXXXX/tables/tblXXXXXXXXXXXXXX/records', {
  method: 'PATCH',
  headers: {
    'Authorization': 'Bearer YOUR_AIRTABLE_TOKEN',
    'Content-Type': 'application/json'
  },
  body: JSON.stringify({
    records: [
      {
        id: 'recXXXXXXXXXXXXXX',
        fields: {
          Status: 'Completed'
        }
      }
    ]
  })
})
.then(response => response.json())
.then(data => console.log(data))
.catch(error => console.error('Error:', error));
```

### Delete Records

```javascript
// Delete records (requires confirmation)
fetch('http://localhost:3000/api/bases/appXXXXXXXXXXXXXX/tables/tblXXXXXXXXXXXXXX/records?records=recXXXXXXXXXXXXXX,recYYYYYYYYYYYYYY', {
  method: 'DELETE',
  headers: {
    'Authorization': 'Bearer YOUR_AIRTABLE_TOKEN',
    'X-Confirm-Destructive-Operation': 'true'
  }
})
.then(response => response.json())
.then(data => console.log(data))
.catch(error => console.error('Error:', error));
```

## Error Handling

The MCP server provides detailed error responses:

```json
{
  "error": {
    "type": "ERROR_TYPE",
    "message": "Human-readable error message",
    "details": {
      // Additional error details
    }
  }
}
```

Common error types:

- `INVALID_REQUEST`: The request is invalid (400)
- `UNAUTHORIZED`: Authentication is required (401)
- `INSUFFICIENT_PERMISSIONS`: The token lacks required scopes (403)
- `FEATURE_UNAVAILABLE`: The feature requires a higher billing plan (403)
- `NOT_FOUND`: The requested resource does not exist (404)
- `CONFIRMATION_REQUIRED`: Confirmation required for destructive operation (400)
- `RATE_LIMIT_REACHED`: Rate limit exceeded (429)
- `SERVER_ERROR`: An unexpected server error occurred (500)

## Best Practices

1. **Use Table IDs**: Always use table IDs instead of names when possible, as names can change.
2. **Batch Operations**: Group operations when possible (up to 10 records per request).
3. **Handle Rate Limits**: Implement backoff strategies when rate limited.
4. **Validate Input**: Validate data before sending it to the API.
5. **Use Partial Updates**: Prefer PATCH over PUT to avoid accidental data loss.
6. **Handle Errors Gracefully**: Parse error responses and handle them appropriately.
7. **Implement Retry Logic**: For network failures and 5xx errors.
