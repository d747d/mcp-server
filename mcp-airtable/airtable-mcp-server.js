// Airtable MCP Server Integration
// A comprehensive Node.js server that integrates with Airtable API
// Features organized by billing plans with safety checks for destructive operations

const express = require('express');
const bodyParser = require('body-parser');
const axios = require('axios');
const cors = require('cors');
const rateLimit = require('express-rate-limit');
const helmet = require('helmet');

const app = express();
const PORT = process.env.PORT || 3000;

// Middleware
app.use(helmet()); // Security headers
app.use(cors());
app.use(bodyParser.json());
app.use(bodyParser.urlencoded({ extended: true }));

// ==========================================
// PROVIDERS
// ==========================================

// AirtableProvider - Core API interaction logic
class AirtableProvider {
  constructor(token) {
    this.baseUrl = 'https://api.airtable.com/v0';
    this.token = token;
    
    // Axios instance with authentication
    this.api = axios.create({
      baseURL: this.baseUrl,
      headers: {
        'Authorization': `Bearer ${token}`,
        'Content-Type': 'application/json'
      }
    });
    
    // Handle rate limiting with exponential backoff
    this.api.interceptors.response.use(
      response => response,
      async error => {
        const { config, response } = error;
        
        // If rate limited (429)
        if (response && response.status === 429) {
          console.log('Rate limited. Backing off...');
          
          // Wait 30 seconds as per Airtable's guidelines
          await new Promise(resolve => setTimeout(resolve, 30000));
          
          // Retry the request
          return this.api(config);
        }
        
        return Promise.reject(error);
      }
    );
  }
  
  // Read operations - available on all plans
  
  async getBases() {
    try {
      const response = await this.api.get('/meta/bases');
      return response.data;
    } catch (error) {
      this._handleError(error);
    }
  }
  
  async getTableRecords(baseId, tableIdOrName, params = {}) {
    try {
      const response = await this.api.get(`/${baseId}/${tableIdOrName}`, { params });
      return response.data;
    } catch (error) {
      this._handleError(error);
    }
  }
  
  async getRecord(baseId, tableIdOrName, recordId) {
    try {
      const response = await this.api.get(`/${baseId}/${tableIdOrName}/${recordId}`);
      return response.data;
    } catch (error) {
      this._handleError(error);
    }
  }
  
  // Write operations - requires proper scopes
  
  async createRecords(baseId, tableIdOrName, records, options = {}) {
    try {
      const response = await this.api.post(`/${baseId}/${tableIdOrName}`, {
        records,
        typecast: options.typecast || false,
        returnFieldsByFieldId: options.returnFieldsByFieldId || false
      });
      return response.data;
    } catch (error) {
      this._handleError(error);
    }
  }
  
  async updateRecords(baseId, tableIdOrName, records, options = {}) {
    try {
      // Use PATCH for partial updates (recommended)
      const method = options.destructive ? 'put' : 'patch';
      
      const response = await this.api[method](`/${baseId}/${tableIdOrName}`, {
        records,
        typecast: options.typecast || false,
        returnFieldsByFieldId: options.returnFieldsByFieldId || false,
        performUpsert: options.performUpsert || undefined
      });
      return response.data;
    } catch (error) {
      this._handleError(error);
    }
  }
  
  async deleteRecords(baseId, tableIdOrName, recordIds) {
    try {
      // Convert single ID to array for consistent handling
      const ids = Array.isArray(recordIds) ? recordIds : [recordIds];
      
      const queryParams = ids.map(id => `records[]=${id}`).join('&');
      const response = await this.api.delete(`/${baseId}/${tableIdOrName}?${queryParams}`);
      return response.data;
    } catch (error) {
      this._handleError(error);
    }
  }
  
  // Webhook operations
  
  async listWebhooks(baseId) {
    try {
      const response = await this.api.get(`/bases/${baseId}/webhooks`);
      return response.data;
    } catch (error) {
      this._handleError(error);
    }
  }
  
  async createWebhook(baseId, options) {
    try {
      const response = await this.api.post(`/bases/${baseId}/webhooks`, options);
      return response.data;
    } catch (error) {
      this._handleError(error);
    }
  }
  
  async deleteWebhook(baseId, webhookId) {
    try {
      const response = await this.api.delete(`/bases/${baseId}/webhooks/${webhookId}`);
      return response.data;
    } catch (error) {
      this._handleError(error);
    }
  }
  
  // Enterprise-specific operations
  
  async getAuditLogs(options = {}) {
    try {
      const response = await this.api.get('/auditLogs', { params: options });
      return response.data;
    } catch (error) {
      this._handleError(error);
    }
  }
  
  async getEnterpriseUsers() {
    try {
      const response = await this.api.get('/enterprise/users');
      return response.data;
    } catch (error) {
      this._handleError(error);
    }
  }
  
  // Schema operations
  
  async getBaseSchema(baseId) {
    try {
      const response = await this.api.get(`/meta/bases/${baseId}/tables`);
      return response.data;
    } catch (error) {
      this._handleError(error);
    }
  }
  
  async createField(baseId, tableId, field) {
    try {
      const response = await this.api.post(`/meta/bases/${baseId}/tables/${tableId}/fields`, field);
      return response.data;
    } catch (error) {
      this._handleError(error);
    }
  }
  
  async updateField(baseId, tableId, fieldId, updates) {
    try {
      const response = await this.api.patch(`/meta/bases/${baseId}/tables/${tableId}/fields/${fieldId}`, updates);
      return response.data;
    } catch (error) {
      this._handleError(error);
    }
  }
  
  // Error handling
  _handleError(error) {
    if (error.response) {
      // Extract Airtable error information
      const statusCode = error.response.status;
      const errorData = error.response.data.error || {};
      
      console.error(`Airtable API Error (${statusCode}):`, errorData);
      
      // Construct a more informative error
      const enhancedError = new Error(errorData.message || 'Airtable API error');
      enhancedError.status = statusCode;
      enhancedError.type = errorData.type;
      enhancedError.details = errorData;
      
      throw enhancedError;
    } else if (error.request) {
      // Request made but no response received
      console.error('Airtable API Request Error:', error.request);
      throw new Error('No response received from Airtable API');
    } else {
      // Error in request setup
      console.error('Request Setup Error:', error.message);
      throw error;
    }
  }
}

// ==========================================
// MODELS
// ==========================================

// BillingPlanModel - Determines feature availability based on billing plan
class BillingPlanModel {
  static PLANS = {
    FREE: 'free',
    TEAMS: 'teams',
    BUSINESS: 'business', 
    ENTERPRISE_SCALE: 'enterprise_scale'
  };
  
  static FEATURES = {
    BASE_DATA: 'base_data',
    VIEWS: 'views',
    SCHEMA_MODIFICATION: 'schema_modification',
    WEBHOOKS: 'webhooks',
    SCIM_USER_MANAGEMENT: 'scim_user_management',
    ENTERPRISE_API: 'enterprise_api',
    CHANGE_EVENTS: 'change_events'
  };
  
  // Features available by plan
  static PLAN_FEATURES = {
    [this.PLANS.FREE]: [
      this.FEATURES.BASE_DATA
    ],
    [this.PLANS.TEAMS]: [
      this.FEATURES.BASE_DATA
    ],
    [this.PLANS.BUSINESS]: [
      this.FEATURES.BASE_DATA,
      this.FEATURES.SCIM_USER_MANAGEMENT
    ],
    [this.PLANS.ENTERPRISE_SCALE]: [
      this.FEATURES.BASE_DATA,
      this.FEATURES.VIEWS,
      this.FEATURES.SCHEMA_MODIFICATION,
      this.FEATURES.WEBHOOKS,
      this.FEATURES.SCIM_USER_MANAGEMENT,
      this.FEATURES.ENTERPRISE_API,
      this.FEATURES.CHANGE_EVENTS
    ]
  };
  
  // Check if a feature is available for a given plan
  static hasFeature(plan, feature) {
    if (!this.PLAN_FEATURES[plan]) {
      throw new Error(`Unknown billing plan: ${plan}`);
    }
    return this.PLAN_FEATURES[plan].includes(feature);
  }
  
  // Get the minimum plan required for a feature
  static getMinimumPlanForFeature(feature) {
    for (const [plan, features] of Object.entries(this.PLAN_FEATURES)) {
      if (features.includes(feature)) {
        return plan;
      }
    }
    
    throw new Error(`Unknown feature: ${feature}`);
  }
}

// RateLimitModel - Handles rate limiting logic based on Airtable limits
class RateLimitModel {
  // Configure rate limiter based on Airtable's limits
  // 5 requests per second per base
  static configureRateLimiter() {
    return rateLimit({
      windowMs: 1000, // 1 second
      max: 5, // 5 requests per window
      standardHeaders: true,
      message: { 
        error: 'RATE_LIMIT_REACHED',
        message: 'Rate limit exceeded. Please try again later.'
      }
    });
  }
  
  // Configure user-level rate limiter (50 requests per second)
  static configureUserRateLimiter() {
    return rateLimit({
      windowMs: 1000, // 1 second
      max: 50, // 50 requests per window
      standardHeaders: true,
      message: {
        error: 'USER_RATE_LIMIT_REACHED',
        message: 'User rate limit exceeded. Please try again later.'
      }
    });
  }
}

// TokenScopeModel - Verifies token scopes against required scopes
class TokenScopeModel {
  static SCOPES = {
    // Basic scopes
    RECORDS_READ: 'data.records:read',
    RECORDS_WRITE: 'data.records:write',
    RECORD_COMMENTS_READ: 'data.recordComments:read',
    RECORD_COMMENTS_WRITE: 'data.recordComments:write',
    SCHEMA_BASES_READ: 'schema.bases:read',
    SCHEMA_BASES_WRITE: 'schema.bases:write',
    WEBHOOK_MANAGE: 'webhook:manage',
    BLOCK_MANAGE: 'block:manage',
    USER_EMAIL_READ: 'user.email:read',
    
    // Enterprise member scopes
    ENTERPRISE_GROUPS_READ: 'enterprise.groups:read',
    WORKSPACES_AND_BASES_READ: 'workspacesAndBases:read',
    WORKSPACES_AND_BASES_WRITE: 'workspacesAndBases:write',
    WORKSPACES_AND_BASES_SHARES_MANAGE: 'workspacesAndBases.shares:manage',
    
    // Enterprise admin scopes
    ENTERPRISE_SCIM_USERS_AND_GROUPS_MANAGE: 'enterprise.scim.usersAndGroups:manage',
    ENTERPRISE_AUDIT_LOGS_READ: 'enterprise.auditLogs:read',
    ENTERPRISE_CHANGE_EVENTS_READ: 'enterprise.changeEvents:read',
    ENTERPRISE_EXPORTS_MANAGE: 'enterprise.exports:manage',
    ENTERPRISE_ACCOUNT_READ: 'enterprise.account:read',
    ENTERPRISE_ACCOUNT_WRITE: 'enterprise.account:write',
    ENTERPRISE_USER_READ: 'enterprise.user:read',
    ENTERPRISE_USER_WRITE: 'enterprise.user:write',
    ENTERPRISE_GROUPS_MANAGE: 'enterprise.groups:manage',
    WORKSPACES_AND_BASES_MANAGE: 'workspacesAndBases:manage'
  };
  
  // Maps endpoints to required scopes
  static ENDPOINT_SCOPES = {
    'getTableRecords': [TokenScopeModel.SCOPES.RECORDS_READ],
    'getRecord': [TokenScopeModel.SCOPES.RECORDS_READ],
    'createRecords': [TokenScopeModel.SCOPES.RECORDS_WRITE],
    'updateRecords': [TokenScopeModel.SCOPES.RECORDS_WRITE],
    'deleteRecords': [TokenScopeModel.SCOPES.RECORDS_WRITE],
    'getComments': [TokenScopeModel.SCOPES.RECORD_COMMENTS_READ],
    'createComment': [TokenScopeModel.SCOPES.RECORD_COMMENTS_WRITE],
    'deleteComment': [TokenScopeModel.SCOPES.RECORD_COMMENTS_WRITE],
    'updateComment': [TokenScopeModel.SCOPES.RECORD_COMMENTS_WRITE],
    'getBases': [TokenScopeModel.SCOPES.SCHEMA_BASES_READ],
    'getBaseSchema': [TokenScopeModel.SCOPES.SCHEMA_BASES_READ],
    'createField': [TokenScopeModel.SCOPES.SCHEMA_BASES_WRITE],
    'updateField': [TokenScopeModel.SCOPES.SCHEMA_BASES_WRITE],
    'listWebhooks': [TokenScopeModel.SCOPES.WEBHOOK_MANAGE],
    'createWebhook': [TokenScopeModel.SCOPES.WEBHOOK_MANAGE],
    'deleteWebhook': [TokenScopeModel.SCOPES.WEBHOOK_MANAGE],
    'getAuditLogs': [TokenScopeModel.SCOPES.ENTERPRISE_AUDIT_LOGS_READ],
    'getEnterpriseUsers': [TokenScopeModel.SCOPES.ENTERPRISE_USER_READ]
  };
  
  // Check if token has required scope
  static hasRequiredScope(tokenScopes, requiredScope) {
    return tokenScopes.includes(requiredScope);
  }
  
  // Check if token has all required scopes for an endpoint
  static hasRequiredScopesForEndpoint(tokenScopes, endpoint) {
    const requiredScopes = this.ENDPOINT_SCOPES[endpoint] || [];
    return requiredScopes.every(scope => this.hasRequiredScope(tokenScopes, scope));
  }
}

// ==========================================
// MIDDLEWARE
// ==========================================

// Authentication middleware
const authMiddleware = async (req, res, next) => {
  try {
    const authHeader = req.headers.authorization;
    
    if (!authHeader || !authHeader.startsWith('Bearer ')) {
      return res.status(401).json({
        error: {
          type: 'INVALID_REQUEST',
          message: 'Authentication required. Please provide a valid token.'
        }
      });
    }
    
    const token = authHeader.split(' ')[1];
    
    if (!token) {
      return res.status(401).json({
        error: {
          type: 'INVALID_REQUEST',
          message: 'Authentication required. Please provide a valid token.'
        }
      });
    }
    
    // Attach the token to the request for later use
    req.token = token;
    
    // In a real implementation, you would validate the token
    // and fetch the user's billing plan and scopes
    
    // For demo purposes, we'll simulate token validation and scope retrieval
    // In production, you would call an Airtable endpoint to validate the token
    
    // Mock billing plan and scopes - in production, these would come from token validation
    req.billingPlan = req.headers['x-billing-plan'] || BillingPlanModel.PLANS.FREE;
    req.tokenScopes = req.headers['x-token-scopes'] ? 
      req.headers['x-token-scopes'].split(',') : 
      [TokenScopeModel.SCOPES.RECORDS_READ];
    
    next();
  } catch (error) {
    console.error('Authentication error:', error);
    res.status(500).json({
      error: {
        type: 'SERVER_ERROR',
        message: 'Failed to authenticate request.'
      }
    });
  }
};

// Feature availability middleware - checks if a feature is available on the user's plan
const featureCheckMiddleware = (feature) => {
  return (req, res, next) => {
    try {
      const { billingPlan } = req;
      
      if (!BillingPlanModel.hasFeature(billingPlan, feature)) {
        const minimumPlan = BillingPlanModel.getMinimumPlanForFeature(feature);
        
        return res.status(403).json({
          error: {
            type: 'FEATURE_UNAVAILABLE',
            message: `This feature requires the ${minimumPlan} plan or higher.`
          }
        });
      }
      
      next();
    } catch (error) {
      console.error('Feature check error:', error);
      res.status(500).json({
        error: {
          type: 'SERVER_ERROR',
          message: 'Failed to check feature availability.'
        }
      });
    }
  };
};

// Scope check middleware - checks if the token has required scopes
const scopeCheckMiddleware = (endpoint) => {
  return (req, res, next) => {
    try {
      const { tokenScopes } = req;
      
      if (!TokenScopeModel.hasRequiredScopesForEndpoint(tokenScopes, endpoint)) {
        const requiredScopes = TokenScopeModel.ENDPOINT_SCOPES[endpoint] || [];
        
        return res.status(403).json({
          error: {
            type: 'INSUFFICIENT_PERMISSIONS',
            message: `This operation requires the following scopes: ${requiredScopes.join(', ')}`
          }
        });
      }
      
      next();
    } catch (error) {
      console.error('Scope check error:', error);
      res.status(500).json({
        error: {
          type: 'SERVER_ERROR',
          message: 'Failed to check token scopes.'
        }
      });
    }
  };
};

// Destructive operation confirmation middleware
const confirmDestructiveOperationMiddleware = (req, res, next) => {
  try {
    const confirmation = req.headers['x-confirm-destructive-operation'];
    
    if (!confirmation || confirmation.toLowerCase() !== 'true') {
      return res.status(400).json({
        error: {
          type: 'CONFIRMATION_REQUIRED',
          message: 'This is a destructive operation. Please confirm by setting the X-Confirm-Destructive-Operation header to "true".'
        }
      });
    }
    
    next();
  } catch (error) {
    console.error('Confirmation check error:', error);
    res.status(500).json({
      error: {
        type: 'SERVER_ERROR',
        message: 'Failed to check operation confirmation.'
      }
    });
  }
};

// Rate limiting middleware (applied to specific routes)
const baseRateLimitMiddleware = RateLimitModel.configureRateLimiter();
const userRateLimitMiddleware = RateLimitModel.configureUserRateLimiter();

// ==========================================
// CONTROLLERS
// ==========================================

// Bases Controller
const basesController = {
  // Get all bases (available to all plans)
  getBases: [
    authMiddleware,
    featureCheckMiddleware(BillingPlanModel.FEATURES.BASE_DATA),
    scopeCheckMiddleware('getBases'),
    userRateLimitMiddleware,
    async (req, res) => {
      try {
        const airtableProvider = new AirtableProvider(req.token);
        const data = await airtableProvider.getBases();
        res.json(data);
      } catch (error) {
        handleErrorResponse(res, error);
      }
    }
  ],
  
  // Get base schema (available to all plans)
  getBaseSchema: [
    authMiddleware,
    featureCheckMiddleware(BillingPlanModel.FEATURES.BASE_DATA),
    scopeCheckMiddleware('getBaseSchema'),
    baseRateLimitMiddleware,
    async (req, res) => {
      try {
        const { baseId } = req.params;
        
        if (!baseId) {
          return res.status(400).json({
            error: {
              type: 'INVALID_REQUEST',
              message: 'Base ID is required'
            }
          });
        }
        
        const airtableProvider = new AirtableProvider(req.token);
        const data = await airtableProvider.getBaseSchema(baseId);
        res.json(data);
      } catch (error) {
        handleErrorResponse(res, error);
      }
    }
  ]
};

// Records Controller
const recordsController = {
  // Get table records (available to all plans)
  getTableRecords: [
    authMiddleware,
    featureCheckMiddleware(BillingPlanModel.FEATURES.BASE_DATA),
    scopeCheckMiddleware('getTableRecords'),
    baseRateLimitMiddleware,
    async (req, res) => {
      try {
        const { baseId, tableIdOrName } = req.params;
        const queryParams = req.query;
        
        if (!baseId || !tableIdOrName) {
          return res.status(400).json({
            error: {
              type: 'INVALID_REQUEST',
              message: 'Base ID and Table ID/Name are required'
            }
          });
        }
        
        const airtableProvider = new AirtableProvider(req.token);
        const data = await airtableProvider.getTableRecords(baseId, tableIdOrName, queryParams);
        res.json(data);
      } catch (error) {
        handleErrorResponse(res, error);
      }
    }
  ],
  
  // Get a single record (available to all plans)
  getRecord: [
    authMiddleware,
    featureCheckMiddleware(BillingPlanModel.FEATURES.BASE_DATA),
    scopeCheckMiddleware('getRecord'),
    baseRateLimitMiddleware,
    async (req, res) => {
      try {
        const { baseId, tableIdOrName, recordId } = req.params;
        
        if (!baseId || !tableIdOrName || !recordId) {
          return res.status(400).json({
            error: {
              type: 'INVALID_REQUEST',
              message: 'Base ID, Table ID/Name, and Record ID are required'
            }
          });
        }
        
        const airtableProvider = new AirtableProvider(req.token);
        const data = await airtableProvider.getRecord(baseId, tableIdOrName, recordId);
        res.json(data);
      } catch (error) {
        handleErrorResponse(res, error);
      }
    }
  ],
  
  // Create records (available to all plans)
  createRecords: [
    authMiddleware,
    featureCheckMiddleware(BillingPlanModel.FEATURES.BASE_DATA),
    scopeCheckMiddleware('createRecords'),
    baseRateLimitMiddleware,
    async (req, res) => {
      try {
        const { baseId, tableIdOrName } = req.params;
        const { records, typecast, returnFieldsByFieldId } = req.body;
        
        if (!baseId || !tableIdOrName) {
          return res.status(400).json({
            error: {
              type: 'INVALID_REQUEST',
              message: 'Base ID and Table ID/Name are required'
            }
          });
        }
        
        if (!records || (Array.isArray(records) && records.length === 0)) {
          return res.status(400).json({
            error: {
              type: 'INVALID_REQUEST',
              message: 'Records are required'
            }
          });
        }
        
        // Check if we're exceeding the maximum records per request (10)
        if (Array.isArray(records) && records.length > 10) {
          return res.status(400).json({
            error: {
              type: 'INVALID_REQUEST',
              message: 'Maximum of 10 records can be created in a single request'
            }
          });
        }
        
        const options = {
          typecast,
          returnFieldsByFieldId
        };
        
        const airtableProvider = new AirtableProvider(req.token);
        const data = await airtableProvider.createRecords(baseId, tableIdOrName, records, options);
        res.json(data);
      } catch (error) {
        handleErrorResponse(res, error);
      }
    }
  ],
  
  // Update records (available to all plans)
  updateRecords: [
    authMiddleware,
    featureCheckMiddleware(BillingPlanModel.FEATURES.BASE_DATA),
    scopeCheckMiddleware('updateRecords'),
    baseRateLimitMiddleware,
    async (req, res) => {
      try {
        const { baseId, tableIdOrName } = req.params;
        const { records, typecast, returnFieldsByFieldId, performUpsert, destructive } = req.body;
        
        if (!baseId || !tableIdOrName) {
          return res.status(400).json({
            error: {
              type: 'INVALID_REQUEST',
              message: 'Base ID and Table ID/Name are required'
            }
          });
        }
        
        if (!records || !Array.isArray(records) || records.length === 0) {
          return res.status(400).json({
            error: {
              type: 'INVALID_REQUEST',
              message: 'Records array is required'
            }
          });
        }
        
        // Check if we're exceeding the maximum records per request (10)
        if (records.length > 10) {
          return res.status(400).json({
            error: {
              type: 'INVALID_REQUEST',
              message: 'Maximum of 10 records can be updated in a single request'
            }
          });
        }
        
        // Additional validation for destructive updates (PUT)
        if (destructive) {
          // Check if the confirmation header is present
          const confirmation = req.headers['x-confirm-destructive-operation'];
          
          if (!confirmation || confirmation.toLowerCase() !== 'true') {
            return res.status(400).json({
              error: {
                type: 'CONFIRMATION_REQUIRED',
                message: 'This is a destructive update (PUT) that will clear all unincluded cell values. Please confirm by setting the X-Confirm-Destructive-Operation header to "true".'
              }
            });
          }
        }
        
        const options = {
          typecast,
          returnFieldsByFieldId,
          performUpsert,
          destructive
        };
        
        const airtableProvider = new AirtableProvider(req.token);
        const data = await airtableProvider.updateRecords(baseId, tableIdOrName, records, options);
        res.json(data);
      } catch (error) {
        handleErrorResponse(res, error);
      }
    }
  ],
  
  // Delete records (available to all plans)
  deleteRecords: [
    authMiddleware,
    featureCheckMiddleware(BillingPlanModel.FEATURES.BASE_DATA),
    scopeCheckMiddleware('deleteRecords'),
    confirmDestructiveOperationMiddleware, // Extra confirmation for deletion
    baseRateLimitMiddleware,
    async (req, res) => {
      try {
        const { baseId, tableIdOrName } = req.params;
        const { records } = req.query;
        
        if (!baseId || !tableIdOrName) {
          return res.status(400).json({
            error: {
              type: 'INVALID_REQUEST',
              message: 'Base ID and Table ID/Name are required'
            }
          });
        }
        
        if (!records) {
          return res.status(400).json({
            error: {
              type: 'INVALID_REQUEST',
              message: 'Record IDs are required'
            }
          });
        }
        
        // Parse the records query parameter
        let recordIds;
        if (Array.isArray(records)) {
          recordIds = records;
        } else if (typeof records === 'string') {
          // If it's a comma-separated string, split it
          recordIds = records.split(',').map(id => id.trim());
        } else {
          return res.status(400).json({
            error: {
              type: 'INVALID_REQUEST',
              message: 'Invalid record IDs format'
            }
          });
        }
        
        // Check if we're exceeding the maximum records per request (10)
        if (recordIds.length > 10) {
          return res.status(400).json({
            error: {
              type: 'INVALID_REQUEST',
              message: 'Maximum of 10 records can be deleted in a single request'
            }
          });
        }
        
        const airtableProvider = new AirtableProvider(req.token);
        const data = await airtableProvider.deleteRecords(baseId, tableIdOrName, recordIds);
        res.json(data);
      } catch (error) {
        handleErrorResponse(res, error);
      }
    }
  ]
};

// Schema Controller (some features limited to paid plans)
const schemaController = {
  // Create field (requires appropriate plan and scope)
  createField: [
    authMiddleware,
    featureCheckMiddleware(BillingPlanModel.FEATURES.SCHEMA_MODIFICATION),
    scopeCheckMiddleware('createField'),
    baseRateLimitMiddleware,
    async (req, res) => {
      try {
        const { baseId, tableId } = req.params;
        const fieldData = req.body;
        
        if (!baseId || !tableId) {
          return res.status(400).json({
            error: {
              type: 'INVALID_REQUEST',
              message: 'Base ID and Table ID are required'
            }
          });
        }
        
        if (!fieldData || !fieldData.name || !fieldData.type) {
          return res.status(400).json({
            error: {
              type: 'INVALID_REQUEST',
              message: 'Field data with name and type is required'
            }
          });
        }
        
        const airtableProvider = new AirtableProvider(req.token);
        const data = await airtableProvider.createField(baseId, tableId, fieldData);
        res.json(data);
      } catch (error) {
        handleErrorResponse(res, error);
      }
    }
  ],
  
  // Update field (requires appropriate plan and scope)
  updateField: [
    authMiddleware,
    featureCheckMiddleware(BillingPlanModel.FEATURES.SCHEMA_MODIFICATION),
    scopeCheckMiddleware('updateField'),
    baseRateLimitMiddleware,
    async (req, res) => {
      try {
        const { baseId, tableId, fieldId } = req.params;
        const updates = req.body;
        
        if (!baseId || !tableId || !fieldId) {
          return res.status(400).json({
            error: {
              type: 'INVALID_REQUEST',
              message: 'Base ID, Table ID, and Field ID are required'
            }
          });
        }
        
        if (!updates || Object.keys(updates).length === 0) {
          return res.status(400).json({
            error: {
              type: 'INVALID_REQUEST',
              message: 'Field updates are required'
            }
          });
        }
        
        const airtableProvider = new AirtableProvider(req.token);
        const data = await airtableProvider.updateField(baseId, tableId, fieldId, updates);
        res.json(data);
      } catch (error) {
        handleErrorResponse(res, error);
      }
    }
  ]
};

// Webhook Controller (limited to plans with webhook feature)
const webhookController = {
  // List webhooks
  listWebhooks: [
    authMiddleware,
    featureCheckMiddleware(BillingPlanModel.FEATURES.WEBHOOKS),
    scopeCheckMiddleware('listWebhooks'),
    baseRateLimitMiddleware,
    async (req, res) => {
      try {
        const { baseId } = req.params;
        
        if (!baseId) {
          return res.status(400).json({
            error: {
              type: 'INVALID_REQUEST',
              message: 'Base ID is required'
            }
          });
        }
        
        const airtableProvider = new AirtableProvider(req.token);
        const data = await airtableProvider.listWebhooks(baseId);
        res.json(data);
      } catch (error) {
        handleErrorResponse(res, error);
      }
    }
  ],
  
  // Create webhook
  createWebhook: [
    authMiddleware,
    featureCheckMiddleware(BillingPlanModel.FEATURES.WEBHOOKS),
    scopeCheckMiddleware('createWebhook'),
    baseRateLimitMiddleware,
    async (req, res) => {
      try {
        const { baseId } = req.params;
        const webhookData = req.body;
        
        if (!baseId) {
          return res.status(400).json({
            error: {
              type: 'INVALID_REQUEST',
              message: 'Base ID is required'
            }
          });
        }
        
        if (!webhookData || !webhookData.notificationUrl) {
          return res.status(400).json({
            error: {
              type: 'INVALID_REQUEST',
              message: 'Webhook notification URL is required'
            }
          });
        }
        
        const airtableProvider = new AirtableProvider(req.token);
        const data = await airtableProvider.createWebhook(baseId, webhookData);
        res.json(data);
      } catch (error) {
        handleErrorResponse(res, error);
      }
    }
  ],
  
  // Delete webhook
  deleteWebhook: [
    authMiddleware,
    featureCheckMiddleware(BillingPlanModel.FEATURES.WEBHOOKS),
    scopeCheckMiddleware('deleteWebhook'),
    confirmDestructiveOperationMiddleware, // Extra confirmation for deletion
    baseRateLimitMiddleware,
    async (req, res) => {
      try {
        const { baseId, webhookId } = req.params;
        
        if (!baseId || !webhookId) {
          return res.status(400).json({
            error: {
              type: 'INVALID_REQUEST',
              message: 'Base ID and Webhook ID are required'
            }
          });
        }
        
        const airtableProvider = new AirtableProvider(req.token);
        const data = await airtableProvider.deleteWebhook(baseId, webhookId);
        res.json(data);
      } catch (error) {
        handleErrorResponse(res, error);
      }
    }
  ]
};

// Enterprise Controller (limited to enterprise plans)
const enterpriseController = {
  // Get audit logs
  getAuditLogs: [
    authMiddleware,
    featureCheckMiddleware(BillingPlanModel.FEATURES.ENTERPRISE_API),
    scopeCheckMiddleware('getAuditLogs'),
    userRateLimitMiddleware,
    async (req, res) => {
      try {
        const options = req.query;
        
        const airtableProvider = new AirtableProvider(req.token);
        const data = await airtableProvider.getAuditLogs(options);
        res.json(data);
      } catch (error) {
        handleErrorResponse(res, error);
      }
    }
  ],
  
  // Get enterprise users
  getEnterpriseUsers: [
    authMiddleware,
    featureCheckMiddleware(BillingPlanModel.FEATURES.ENTERPRISE_API),
    scopeCheckMiddleware('getEnterpriseUsers'),
    userRateLimitMiddleware,
    async (req, res) => {
      try {
        const airtableProvider = new AirtableProvider(req.token);
        const data = await airtableProvider.getEnterpriseUsers();
        res.json(data);
      } catch (error) {
        handleErrorResponse(res, error);
      }
    }
  ]
};

// Utility function to handle error responses
const handleErrorResponse = (res, error) => {
  console.error('API Error:', error);
  
  // If it's an error we've enhanced with status and type
  if (error.status && error.type) {
    return res.status(error.status).json({
      error: {
        type: error.type,
        message: error.message,
        details: error.details
      }
    });
  }
  
  // Default error response
  res.status(500).json({
    error: {
      type: 'SERVER_ERROR',
      message: error.message || 'An unexpected error occurred'
    }
  });
};

// ==========================================
// ROUTES
// ==========================================

// Bases routes
app.get('/api/bases', basesController.getBases);
app.get('/api/bases/:baseId/schema', basesController.getBaseSchema);

// Records routes
app.get('/api/bases/:baseId/tables/:tableIdOrName/records', recordsController.getTableRecords);
app.get('/api/bases/:baseId/tables/:tableIdOrName/records/:recordId', recordsController.getRecord);
app.post('/api/bases/:baseId/tables/:tableIdOrName/records', recordsController.createRecords);
app.patch('/api/bases/:baseId/tables/:tableIdOrName/records', recordsController.updateRecords);
app.delete('/api/bases/:baseId/tables/:tableIdOrName/records', recordsController.deleteRecords);

// Schema routes (limited by plan)
app.post('/api/bases/:baseId/tables/:tableId/fields', schemaController.createField);
app.patch('/api/bases/:baseId/tables/:tableId/fields/:fieldId', schemaController.updateField);

// Webhook routes (limited by plan)
app.get('/api/bases/:baseId/webhooks', webhookController.listWebhooks);
app.post('/api/bases/:baseId/webhooks', webhookController.createWebhook);
app.delete('/api/bases/:baseId/webhooks/:webhookId', webhookController.deleteWebhook);

// Enterprise routes (limited by plan)
app.get('/api/enterprise/audit-logs', enterpriseController.getAuditLogs);
app.get('/api/enterprise/users', enterpriseController.getEnterpriseUsers);

// Health check route
app.get('/api/health', (req, res) => {
  res.json({ status: 'ok', version: '1.0.0' });
});

// Catch-all for undefined routes
app.use((req, res) => {
  res.status(404).json({
    error: {
      type: 'NOT_FOUND',
      message: 'The requested endpoint does not exist.'
    }
  });
});

// Error handling middleware
app.use((error, req, res, next) => {
  console.error('Unhandled Error:', error);
  
  res.status(500).json({
    error: {
      type: 'SERVER_ERROR',
      message: 'An unexpected server error occurred.'
    }
  });
});

// ==========================================
// START SERVER
// ==========================================

app.listen(PORT, () => {
  console.log(`Airtable MCP Server running on port ${PORT}`);
});

module.exports = app; // For testing purposes
