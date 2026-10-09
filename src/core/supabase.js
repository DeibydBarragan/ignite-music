'use strict';

const { createClient } = require('@supabase/supabase-js');
const logger = require('./logger');

const log = logger.createLogger('supabase');

const SUPABASE_URL = process.env.SUPABASE_URL || 'https://qzedjwrpvcgmfqgsvxyi.supabase.co';
const SUPABASE_KEY =
    process.env.SUPABASE_KEY ||
    process.env.SUPABASE_ANON_KEY ||
    'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6InF6ZWRqd3JwdmNnbWZxZ3N2eHlpIiwicm9sZSI6ImFub24iLCJpYXQiOjE3OTEwMzI3MDAsImV4cCI6MjEwNjYwODcwMH0.0XJ6R25ZXcS31Et6ugu-dCWSh0JsJeAIKiKh1XGuEIg';

let supabase = null;

try {
    if (SUPABASE_URL && SUPABASE_KEY) {
        supabase = createClient(SUPABASE_URL, SUPABASE_KEY);
        log.ok('Supabase client initialized successfully');
    } else {
        log.warn('Missing SUPABASE_URL or SUPABASE_KEY');
    }
} catch (err) {
    log.error('Failed to initialize Supabase client:', err.message);
}

module.exports = { supabase };
