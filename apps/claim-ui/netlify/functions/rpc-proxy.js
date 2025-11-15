/**
 * Netlify Function: RPC Proxy
 *
 * Proxies Solana RPC requests to keep the API key server-side.
 * This prevents exposing your RPC provider key in the frontend bundle.
 */

export async function handler(event, context) {
  // CORS headers for browser requests
  const corsHeaders = {
    'Access-Control-Allow-Origin': '*',
    'Access-Control-Allow-Headers': 'content-type',
    'Access-Control-Allow-Methods': 'POST, OPTIONS',
  };

  // Handle OPTIONS preflight
  if (event.httpMethod === 'OPTIONS') {
    return {
      statusCode: 204,
      headers: corsHeaders,
      body: '',
    };
  }

  // Only allow POST
  if (event.httpMethod !== 'POST') {
    return {
      statusCode: 405,
      headers: corsHeaders,
      body: JSON.stringify({ error: 'Method not allowed' }),
    };
  }

  // Get config from environment
  const { RPC_URL, CLAIM_UI_KEY, AUTH_MODE } = process.env;

  if (!RPC_URL || !CLAIM_UI_KEY) {
    console.error('Missing required environment variables: RPC_URL or CLAIM_UI_KEY');
    return {
      statusCode: 500,
      headers: corsHeaders,
      body: JSON.stringify({ error: 'Proxy not configured' }),
    };
  }

  try {
    // Prepare headers for upstream RPC request
    const upstreamHeaders = {
      'content-type': 'application/json',
    };

    // Add auth header based on AUTH_MODE
    if (AUTH_MODE === 'bearer') {
      upstreamHeaders.authorization = `Bearer ${CLAIM_UI_KEY}`;
    } else {
      // Default: x-api-key header
      upstreamHeaders['x-api-key'] = CLAIM_UI_KEY;
    }

    // Forward request to RPC provider
    const response = await fetch(RPC_URL, {
      method: 'POST',
      headers: upstreamHeaders,
      body: event.body,
    });

    const responseText = await response.text();

    // Return proxied response
    return {
      statusCode: response.status,
      headers: {
        ...corsHeaders,
        'content-type': 'application/json',
      },
      body: responseText,
    };
  } catch (error) {
    console.error('RPC proxy error:', error);
    return {
      statusCode: 500,
      headers: corsHeaders,
      body: JSON.stringify({
        error: 'RPC request failed',
        message: error.message,
      }),
    };
  }
}
