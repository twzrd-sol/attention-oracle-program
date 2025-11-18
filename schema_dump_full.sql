--
-- PostgreSQL database dump
--

\restrict 84K8KKI21DWmztjRdNAy3As2c0zumJJGhrnI5qz5JdQqQgPAUxafoaeW6jJVIs3

-- Dumped from database version 14.19 (Ubuntu 14.19-0ubuntu0.22.04.1)
-- Dumped by pg_dump version 14.19 (Ubuntu 14.19-0ubuntu0.22.04.1)

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Name: pg_stat_statements; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS pg_stat_statements WITH SCHEMA public;


--
-- Name: EXTENSION pg_stat_statements; Type: COMMENT; Schema: -; Owner: 
--

COMMENT ON EXTENSION pg_stat_statements IS 'track planning and execution statistics of all SQL statements executed';


--
-- Name: pgstattuple; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS pgstattuple WITH SCHEMA public;


--
-- Name: EXTENSION pgstattuple; Type: COMMENT; Schema: -; Owner: 
--

COMMENT ON EXTENSION pgstattuple IS 'show tuple-level statistics';


SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: allocations; Type: TABLE; Schema: public; Owner: twzrd
--

CREATE TABLE public.allocations (
    epoch_id integer NOT NULL,
    wallet text NOT NULL,
    index integer NOT NULL,
    amount bigint NOT NULL,
    id text,
    proof_json text NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.allocations OWNER TO twzrd;

--
-- Name: attention_index; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.attention_index (
    epoch bigint NOT NULL,
    channel text NOT NULL,
    value real NOT NULL,
    participants integer NOT NULL,
    messages integer NOT NULL,
    computed_at bigint NOT NULL
);


ALTER TABLE public.attention_index OWNER TO postgres;

--
-- Name: channel_participation; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.channel_participation (
    epoch bigint NOT NULL,
    channel text NOT NULL,
    user_hash text NOT NULL,
    first_seen bigint NOT NULL,
    token_group text DEFAULT 'MILO'::text NOT NULL,
    category text DEFAULT 'default'::text NOT NULL
);


ALTER TABLE public.channel_participation OWNER TO postgres;

--
-- Name: channel_payouts; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.channel_payouts (
    epoch bigint NOT NULL,
    channel text NOT NULL,
    participant_count integer NOT NULL,
    total_weight double precision NOT NULL,
    viewer_amount bigint NOT NULL,
    streamer_amount bigint NOT NULL,
    viewer_ratio double precision NOT NULL,
    streamer_ratio double precision NOT NULL,
    updated_at bigint NOT NULL
);


ALTER TABLE public.channel_payouts OWNER TO postgres;

--
-- Name: chatter_snapshots; Type: TABLE; Schema: public; Owner: twzrd
--

CREATE TABLE public.chatter_snapshots (
    id integer NOT NULL,
    stream_id character varying(255) NOT NULL,
    minute_ts timestamp without time zone NOT NULL,
    viewer_ids jsonb NOT NULL,
    viewer_count integer NOT NULL,
    created_at timestamp without time zone DEFAULT now()
);


ALTER TABLE public.chatter_snapshots OWNER TO twzrd;

--
-- Name: chatter_snapshots_id_seq; Type: SEQUENCE; Schema: public; Owner: twzrd
--

CREATE SEQUENCE public.chatter_snapshots_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE public.chatter_snapshots_id_seq OWNER TO twzrd;

--
-- Name: chatter_snapshots_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: twzrd
--

ALTER SEQUENCE public.chatter_snapshots_id_seq OWNED BY public.chatter_snapshots.id;


--
-- Name: cls_claims; Type: TABLE; Schema: public; Owner: twzrd
--

CREATE TABLE public.cls_claims (
    id bigint NOT NULL,
    wallet text NOT NULL,
    epoch_id integer NOT NULL,
    amount bigint,
    tx_signature text,
    tx_status character varying(20) DEFAULT 'pending'::character varying,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    confirmed_at timestamp with time zone
);


ALTER TABLE public.cls_claims OWNER TO twzrd;

--
-- Name: cls_claims_id_seq; Type: SEQUENCE; Schema: public; Owner: twzrd
--

CREATE SEQUENCE public.cls_claims_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE public.cls_claims_id_seq OWNER TO twzrd;

--
-- Name: cls_claims_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: twzrd
--

ALTER SEQUENCE public.cls_claims_id_seq OWNED BY public.cls_claims.id;


--
-- Name: cls_discovered_channels; Type: TABLE; Schema: public; Owner: twzrd
--

CREATE TABLE public.cls_discovered_channels (
    id integer NOT NULL,
    channel_name text NOT NULL,
    viewer_count integer NOT NULL,
    rank integer NOT NULL,
    discovered_at bigint NOT NULL,
    discovery_run_id text NOT NULL,
    metadata jsonb,
    created_at timestamp without time zone DEFAULT now(),
    category text
);


ALTER TABLE public.cls_discovered_channels OWNER TO twzrd;

--
-- Name: TABLE cls_discovered_channels; Type: COMMENT; Schema: public; Owner: twzrd
--

COMMENT ON TABLE public.cls_discovered_channels IS 'Audit trail of CLS Top 100 discovery runs (hourly)';


--
-- Name: cls_current_top100; Type: VIEW; Schema: public; Owner: twzrd
--

CREATE VIEW public.cls_current_top100 AS
 SELECT DISTINCT ON (cls_discovered_channels.channel_name) cls_discovered_channels.channel_name,
    cls_discovered_channels.viewer_count,
    cls_discovered_channels.rank,
    cls_discovered_channels.discovered_at,
    cls_discovered_channels.metadata
   FROM public.cls_discovered_channels
  WHERE (cls_discovered_channels.discovery_run_id = ( SELECT cls_discovered_channels_1.discovery_run_id
           FROM public.cls_discovered_channels cls_discovered_channels_1
          ORDER BY cls_discovered_channels_1.discovered_at DESC
         LIMIT 1))
  ORDER BY cls_discovered_channels.channel_name, cls_discovered_channels.discovered_at DESC;


ALTER TABLE public.cls_current_top100 OWNER TO twzrd;

--
-- Name: VIEW cls_current_top100; Type: COMMENT; Schema: public; Owner: twzrd
--

COMMENT ON VIEW public.cls_current_top100 IS 'Currently active Top 100 channels from latest discovery';


--
-- Name: cls_discovered_channels_id_seq; Type: SEQUENCE; Schema: public; Owner: twzrd
--

CREATE SEQUENCE public.cls_discovered_channels_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE public.cls_discovered_channels_id_seq OWNER TO twzrd;

--
-- Name: cls_discovered_channels_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: twzrd
--

ALTER SEQUENCE public.cls_discovered_channels_id_seq OWNED BY public.cls_discovered_channels.id;


--
-- Name: epochs; Type: TABLE; Schema: public; Owner: twzrd
--

CREATE TABLE public.epochs (
    id integer NOT NULL,
    streamer_username character varying(255) NOT NULL,
    epoch_start bigint NOT NULL,
    epoch_end bigint NOT NULL,
    merkle_root character varying(64) NOT NULL,
    leaf_count integer NOT NULL,
    created_at timestamp without time zone DEFAULT now(),
    epoch_id integer,
    is_open boolean DEFAULT true NOT NULL
);


ALTER TABLE public.epochs OWNER TO twzrd;

--
-- Name: epochs_id_seq; Type: SEQUENCE; Schema: public; Owner: twzrd
--

CREATE SEQUENCE public.epochs_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE public.epochs_id_seq OWNER TO twzrd;

--
-- Name: epochs_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: twzrd
--

ALTER SEQUENCE public.epochs_id_seq OWNED BY public.epochs.id;


--
-- Name: l2_tree_cache; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.l2_tree_cache (
    epoch bigint NOT NULL,
    channel text NOT NULL,
    root text NOT NULL,
    levels_json text NOT NULL,
    participant_count integer NOT NULL,
    built_at bigint NOT NULL
);


ALTER TABLE public.l2_tree_cache OWNER TO postgres;

--
-- Name: live_windows; Type: TABLE; Schema: public; Owner: twzrd
--

CREATE TABLE public.live_windows (
    channel text NOT NULL,
    start_ts bigint NOT NULL,
    end_ts bigint
);


ALTER TABLE public.live_windows OWNER TO twzrd;

--
-- Name: merkle_roots; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.merkle_roots (
    id integer NOT NULL,
    channel text NOT NULL,
    epoch bigint NOT NULL,
    merkle_root text NOT NULL,
    total_amount bigint DEFAULT 0 NOT NULL,
    claim_count integer DEFAULT 0 NOT NULL,
    sealed boolean DEFAULT false,
    created_at timestamp without time zone DEFAULT CURRENT_TIMESTAMP,
    sealed_at timestamp without time zone
);


ALTER TABLE public.merkle_roots OWNER TO postgres;

--
-- Name: merkle_roots_id_seq; Type: SEQUENCE; Schema: public; Owner: postgres
--

CREATE SEQUENCE public.merkle_roots_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE public.merkle_roots_id_seq OWNER TO postgres;

--
-- Name: merkle_roots_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: postgres
--

ALTER SEQUENCE public.merkle_roots_id_seq OWNED BY public.merkle_roots.id;


--
-- Name: merkle_tree_metadata; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.merkle_tree_metadata (
    id integer NOT NULL,
    channel text NOT NULL,
    epoch bigint NOT NULL,
    tree_height integer NOT NULL,
    leaf_count integer NOT NULL,
    metadata jsonb,
    created_at timestamp without time zone DEFAULT CURRENT_TIMESTAMP
);


ALTER TABLE public.merkle_tree_metadata OWNER TO postgres;

--
-- Name: merkle_tree_metadata_id_seq; Type: SEQUENCE; Schema: public; Owner: postgres
--

CREATE SEQUENCE public.merkle_tree_metadata_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE public.merkle_tree_metadata_id_seq OWNER TO postgres;

--
-- Name: merkle_tree_metadata_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: postgres
--

ALTER SEQUENCE public.merkle_tree_metadata_id_seq OWNED BY public.merkle_tree_metadata.id;


--
-- Name: payout_logs; Type: TABLE; Schema: public; Owner: twzrd
--

CREATE TABLE public.payout_logs (
    id integer NOT NULL,
    viewer_id character varying(255) NOT NULL,
    wallet character varying(44) NOT NULL,
    tokens numeric(20,6) NOT NULL,
    signature character varying(88),
    error text,
    status character varying(20) NOT NULL,
    created_at timestamp without time zone DEFAULT now()
);


ALTER TABLE public.payout_logs OWNER TO twzrd;

--
-- Name: payout_logs_id_seq; Type: SEQUENCE; Schema: public; Owner: twzrd
--

CREATE SEQUENCE public.payout_logs_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE public.payout_logs_id_seq OWNER TO twzrd;

--
-- Name: payout_logs_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: twzrd
--

ALTER SEQUENCE public.payout_logs_id_seq OWNED BY public.payout_logs.id;


--
-- Name: processed_payouts; Type: TABLE; Schema: public; Owner: twzrd
--

CREATE TABLE public.processed_payouts (
    idempotency_key character varying(64) NOT NULL,
    epoch_id character varying(255) NOT NULL,
    root character varying(64) NOT NULL,
    rewards jsonb NOT NULL,
    signatures jsonb NOT NULL,
    processed_at timestamp without time zone NOT NULL
);


ALTER TABLE public.processed_payouts OWNER TO twzrd;

--
-- Name: reward_epochs; Type: TABLE; Schema: public; Owner: twzrd
--

CREATE TABLE public.reward_epochs (
    epoch_id character varying(255) NOT NULL,
    stream_id character varying(255) NOT NULL,
    start_time timestamp without time zone NOT NULL,
    end_time timestamp without time zone NOT NULL,
    total_viewers integer DEFAULT 0,
    total_minutes integer DEFAULT 0,
    total_tokens numeric(20,6) DEFAULT 0,
    merkle_root character varying(64),
    finalized boolean DEFAULT false,
    created_at timestamp without time zone DEFAULT now()
);


ALTER TABLE public.reward_epochs OWNER TO twzrd;

--
-- Name: sealed_epochs; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.sealed_epochs (
    epoch bigint NOT NULL,
    channel text NOT NULL,
    root text NOT NULL,
    sealed_at bigint NOT NULL,
    published integer DEFAULT 0,
    token_group text DEFAULT 'MILO'::text NOT NULL,
    category text DEFAULT 'default'::text NOT NULL,
    published_at timestamp with time zone
);


ALTER TABLE public.sealed_epochs OWNER TO postgres;

--
-- Name: sealed_participants; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.sealed_participants (
    epoch bigint NOT NULL,
    channel text NOT NULL,
    idx integer NOT NULL,
    user_hash text NOT NULL,
    username text,
    token_group text DEFAULT 'MILO'::text NOT NULL,
    category text DEFAULT 'default'::text NOT NULL
);


ALTER TABLE public.sealed_participants OWNER TO postgres;

--
-- Name: social_verification; Type: TABLE; Schema: public; Owner: twzrd
--

CREATE TABLE public.social_verification (
    wallet text NOT NULL,
    twitter_handle text,
    twitter_followed boolean DEFAULT true NOT NULL,
    discord_id text,
    discord_joined boolean DEFAULT true NOT NULL,
    passport_tier integer,
    updated_by text,
    update_reason text,
    last_verified timestamp with time zone DEFAULT now() NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);


ALTER TABLE public.social_verification OWNER TO twzrd;

--
-- Name: streams; Type: TABLE; Schema: public; Owner: twzrd
--

CREATE TABLE public.streams (
    stream_id character varying(255) NOT NULL,
    broadcaster_id character varying(255) NOT NULL,
    started_at timestamp without time zone NOT NULL,
    ended_at timestamp without time zone,
    status character varying(20) DEFAULT 'online'::character varying,
    processed boolean DEFAULT false,
    created_at timestamp without time zone DEFAULT now()
);


ALTER TABLE public.streams OWNER TO twzrd;

--
-- Name: submissions; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.submissions (
    id integer NOT NULL,
    user_pubkey text NOT NULL,
    channel text NOT NULL,
    epoch bigint NOT NULL,
    amount bigint NOT NULL,
    proof jsonb NOT NULL,
    leaf_index integer NOT NULL,
    user_id text,
    created_at timestamp without time zone DEFAULT CURRENT_TIMESTAMP
);


ALTER TABLE public.submissions OWNER TO postgres;

--
-- Name: submissions_id_seq; Type: SEQUENCE; Schema: public; Owner: postgres
--

CREATE SEQUENCE public.submissions_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE public.submissions_id_seq OWNER TO postgres;

--
-- Name: submissions_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: postgres
--

ALTER SEQUENCE public.submissions_id_seq OWNED BY public.submissions.id;


--
-- Name: suppression_list; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.suppression_list (
    user_hash text NOT NULL,
    username text NOT NULL,
    requested_at bigint NOT NULL,
    reason text,
    ip_hash text,
    created_at timestamp without time zone DEFAULT CURRENT_TIMESTAMP
);


ALTER TABLE public.suppression_list OWNER TO postgres;

--
-- Name: suppression_log; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.suppression_log (
    id integer NOT NULL,
    user_hash text NOT NULL,
    username text NOT NULL,
    action text NOT NULL,
    requested_at bigint NOT NULL,
    ip_hash text,
    created_at timestamp without time zone DEFAULT CURRENT_TIMESTAMP
);


ALTER TABLE public.suppression_log OWNER TO postgres;

--
-- Name: suppression_log_id_seq; Type: SEQUENCE; Schema: public; Owner: postgres
--

CREATE SEQUENCE public.suppression_log_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE public.suppression_log_id_seq OWNER TO postgres;

--
-- Name: suppression_log_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: postgres
--

ALTER SEQUENCE public.suppression_log_id_seq OWNED BY public.suppression_log.id;


--
-- Name: system_config; Type: TABLE; Schema: public; Owner: twzrd
--

CREATE TABLE public.system_config (
    key character varying(255) NOT NULL,
    value text NOT NULL,
    updated_at timestamp without time zone DEFAULT now()
);


ALTER TABLE public.system_config OWNER TO twzrd;

--
-- Name: twitch_events; Type: TABLE; Schema: public; Owner: twzrd
--

CREATE TABLE public.twitch_events (
    message_id character varying(255) NOT NULL,
    event_type character varying(50) NOT NULL,
    payload jsonb NOT NULL,
    created_at timestamp without time zone DEFAULT now()
);


ALTER TABLE public.twitch_events OWNER TO twzrd;

--
-- Name: twitch_tokens; Type: TABLE; Schema: public; Owner: twzrd
--

CREATE TABLE public.twitch_tokens (
    user_id character varying(255) NOT NULL,
    username character varying(255) NOT NULL,
    access_token text NOT NULL,
    refresh_token text NOT NULL,
    expires_at timestamp without time zone NOT NULL,
    created_at timestamp without time zone DEFAULT now(),
    updated_at timestamp without time zone DEFAULT now()
);


ALTER TABLE public.twitch_tokens OWNER TO twzrd;

--
-- Name: twitch_wallet_bindings; Type: TABLE; Schema: public; Owner: twzrd
--

CREATE TABLE public.twitch_wallet_bindings (
    twitch_id text NOT NULL,
    login text NOT NULL,
    wallet text NOT NULL,
    created_at timestamp with time zone DEFAULT now(),
    updated_at timestamp with time zone DEFAULT now()
);


ALTER TABLE public.twitch_wallet_bindings OWNER TO twzrd;

--
-- Name: user_mapping; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.user_mapping (
    user_hash text NOT NULL,
    username text NOT NULL,
    first_seen bigint NOT NULL
);


ALTER TABLE public.user_mapping OWNER TO postgres;

--
-- Name: user_signals; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.user_signals (
    epoch bigint NOT NULL,
    channel text NOT NULL,
    user_hash text NOT NULL,
    signal_type text NOT NULL,
    value real NOT NULL,
    "timestamp" bigint NOT NULL
);


ALTER TABLE public.user_signals OWNER TO postgres;

--
-- Name: user_wallets; Type: TABLE; Schema: public; Owner: twzrd
--

CREATE TABLE public.user_wallets (
    twitch_user_id character varying(255) NOT NULL,
    wallet_address character varying(44) NOT NULL,
    bound_at timestamp without time zone NOT NULL,
    updated_at timestamp without time zone DEFAULT now(),
    verified boolean DEFAULT false
);


ALTER TABLE public.user_wallets OWNER TO twzrd;

--
-- Name: v_payout_health; Type: VIEW; Schema: public; Owner: twzrd
--

CREATE VIEW public.v_payout_health AS
 SELECT ( SELECT count(*) AS count
           FROM public.processed_payouts) AS batches_total,
    ( SELECT count(*) AS count
           FROM public.payout_logs
          WHERE (((payout_logs.status)::text = 'failed'::text) AND (payout_logs.created_at > (now() - '1 day'::interval)))) AS failed_24h,
    ( SELECT count(*) AS count
           FROM public.payout_logs
          WHERE (((payout_logs.status)::text = 'success'::text) AND (payout_logs.created_at > (now() - '1 day'::interval)))) AS success_24h;


ALTER TABLE public.v_payout_health OWNER TO twzrd;

--
-- Name: v_stream_status; Type: VIEW; Schema: public; Owner: twzrd
--

CREATE VIEW public.v_stream_status AS
 SELECT s.broadcaster_id,
    s.stream_id,
    s.status,
    s.started_at,
    s.ended_at,
    ( SELECT max(cs.minute_ts) AS max
           FROM public.chatter_snapshots cs
          WHERE ((cs.stream_id)::text = (s.stream_id)::text)) AS last_snapshot,
    (EXTRACT(epoch FROM (now() - (( SELECT max(cs.minute_ts) AS max
           FROM public.chatter_snapshots cs
          WHERE ((cs.stream_id)::text = (s.stream_id)::text)))::timestamp with time zone)))::integer AS snapshot_lag_sec
   FROM public.streams s;


ALTER TABLE public.v_stream_status OWNER TO twzrd;

--
-- Name: viewer_activity; Type: TABLE; Schema: public; Owner: twzrd
--

CREATE TABLE public.viewer_activity (
    viewer_id character varying(255) NOT NULL,
    stream_id character varying(255) NOT NULL,
    join_count integer DEFAULT 0,
    leave_count integer DEFAULT 0,
    total_minutes integer DEFAULT 0,
    flagged boolean DEFAULT false
);


ALTER TABLE public.viewer_activity OWNER TO twzrd;

--
-- Name: viewer_snapshots; Type: TABLE; Schema: public; Owner: postgres
--

CREATE TABLE public.viewer_snapshots (
    id integer NOT NULL,
    epoch_id bigint NOT NULL,
    streamer_name character varying(255) NOT NULL,
    total_viewers integer DEFAULT 0 NOT NULL,
    qualified_viewers integer DEFAULT 0 NOT NULL,
    quality_score numeric(5,3) DEFAULT 0 NOT NULL,
    merkle_root character varying(64),
    created_at timestamp without time zone DEFAULT CURRENT_TIMESTAMP
);


ALTER TABLE public.viewer_snapshots OWNER TO postgres;

--
-- Name: viewer_snapshots_id_seq; Type: SEQUENCE; Schema: public; Owner: postgres
--

CREATE SEQUENCE public.viewer_snapshots_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


ALTER TABLE public.viewer_snapshots_id_seq OWNER TO postgres;

--
-- Name: viewer_snapshots_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: postgres
--

ALTER SEQUENCE public.viewer_snapshots_id_seq OWNED BY public.viewer_snapshots.id;


--
-- Name: weighted_participants; Type: TABLE; Schema: public; Owner: twzrd
--

CREATE TABLE public.weighted_participants (
    channel text NOT NULL,
    epoch bigint NOT NULL,
    user_hash text NOT NULL,
    weight integer NOT NULL
);


ALTER TABLE public.weighted_participants OWNER TO twzrd;

--
-- Name: chatter_snapshots id; Type: DEFAULT; Schema: public; Owner: twzrd
--

ALTER TABLE ONLY public.chatter_snapshots ALTER COLUMN id SET DEFAULT nextval('public.chatter_snapshots_id_seq'::regclass);


--
-- Name: cls_claims id; Type: DEFAULT; Schema: public; Owner: twzrd
--

ALTER TABLE ONLY public.cls_claims ALTER COLUMN id SET DEFAULT nextval('public.cls_claims_id_seq'::regclass);


--
-- Name: cls_discovered_channels id; Type: DEFAULT; Schema: public; Owner: twzrd
--

ALTER TABLE ONLY public.cls_discovered_channels ALTER COLUMN id SET DEFAULT nextval('public.cls_discovered_channels_id_seq'::regclass);


--
-- Name: epochs id; Type: DEFAULT; Schema: public; Owner: twzrd
--

ALTER TABLE ONLY public.epochs ALTER COLUMN id SET DEFAULT nextval('public.epochs_id_seq'::regclass);


--
-- Name: merkle_roots id; Type: DEFAULT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.merkle_roots ALTER COLUMN id SET DEFAULT nextval('public.merkle_roots_id_seq'::regclass);


--
-- Name: merkle_tree_metadata id; Type: DEFAULT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.merkle_tree_metadata ALTER COLUMN id SET DEFAULT nextval('public.merkle_tree_metadata_id_seq'::regclass);


--
-- Name: payout_logs id; Type: DEFAULT; Schema: public; Owner: twzrd
--

ALTER TABLE ONLY public.payout_logs ALTER COLUMN id SET DEFAULT nextval('public.payout_logs_id_seq'::regclass);


--
-- Name: submissions id; Type: DEFAULT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.submissions ALTER COLUMN id SET DEFAULT nextval('public.submissions_id_seq'::regclass);


--
-- Name: suppression_log id; Type: DEFAULT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.suppression_log ALTER COLUMN id SET DEFAULT nextval('public.suppression_log_id_seq'::regclass);


--
-- Name: viewer_snapshots id; Type: DEFAULT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.viewer_snapshots ALTER COLUMN id SET DEFAULT nextval('public.viewer_snapshots_id_seq'::regclass);


--
-- Name: allocations allocations_epoch_id_wallet_key; Type: CONSTRAINT; Schema: public; Owner: twzrd
--

ALTER TABLE ONLY public.allocations
    ADD CONSTRAINT allocations_epoch_id_wallet_key UNIQUE (epoch_id, wallet);


--
-- Name: attention_index attention_index_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.attention_index
    ADD CONSTRAINT attention_index_pkey PRIMARY KEY (epoch, channel);


--
-- Name: channel_participation channel_participation_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.channel_participation
    ADD CONSTRAINT channel_participation_pkey PRIMARY KEY (epoch, channel, user_hash);


--
-- Name: channel_payouts channel_payouts_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.channel_payouts
    ADD CONSTRAINT channel_payouts_pkey PRIMARY KEY (epoch, channel);


--
-- Name: chatter_snapshots chatter_snapshots_pkey; Type: CONSTRAINT; Schema: public; Owner: twzrd
--

ALTER TABLE ONLY public.chatter_snapshots
    ADD CONSTRAINT chatter_snapshots_pkey PRIMARY KEY (id);


--
-- Name: chatter_snapshots chatter_snapshots_stream_id_minute_ts_key; Type: CONSTRAINT; Schema: public; Owner: twzrd
--

ALTER TABLE ONLY public.chatter_snapshots
    ADD CONSTRAINT chatter_snapshots_stream_id_minute_ts_key UNIQUE (stream_id, minute_ts);


--
-- Name: cls_claims cls_claims_pkey; Type: CONSTRAINT; Schema: public; Owner: twzrd
--

ALTER TABLE ONLY public.cls_claims
    ADD CONSTRAINT cls_claims_pkey PRIMARY KEY (id);


--
-- Name: cls_claims cls_claims_wallet_epoch_id_key; Type: CONSTRAINT; Schema: public; Owner: twzrd
--

ALTER TABLE ONLY public.cls_claims
    ADD CONSTRAINT cls_claims_wallet_epoch_id_key UNIQUE (wallet, epoch_id);


--
-- Name: cls_discovered_channels cls_discovered_channels_pkey; Type: CONSTRAINT; Schema: public; Owner: twzrd
--

ALTER TABLE ONLY public.cls_discovered_channels
    ADD CONSTRAINT cls_discovered_channels_pkey PRIMARY KEY (id);


--
-- Name: epochs epochs_pkey; Type: CONSTRAINT; Schema: public; Owner: twzrd
--

ALTER TABLE ONLY public.epochs
    ADD CONSTRAINT epochs_pkey PRIMARY KEY (id);


--
-- Name: l2_tree_cache l2_tree_cache_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.l2_tree_cache
    ADD CONSTRAINT l2_tree_cache_pkey PRIMARY KEY (epoch, channel);


--
-- Name: live_windows live_windows_pkey; Type: CONSTRAINT; Schema: public; Owner: twzrd
--

ALTER TABLE ONLY public.live_windows
    ADD CONSTRAINT live_windows_pkey PRIMARY KEY (channel, start_ts);


--
-- Name: merkle_roots merkle_roots_channel_epoch_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.merkle_roots
    ADD CONSTRAINT merkle_roots_channel_epoch_key UNIQUE (channel, epoch);


--
-- Name: merkle_roots merkle_roots_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.merkle_roots
    ADD CONSTRAINT merkle_roots_pkey PRIMARY KEY (id);


--
-- Name: merkle_tree_metadata merkle_tree_metadata_channel_epoch_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.merkle_tree_metadata
    ADD CONSTRAINT merkle_tree_metadata_channel_epoch_key UNIQUE (channel, epoch);


--
-- Name: merkle_tree_metadata merkle_tree_metadata_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.merkle_tree_metadata
    ADD CONSTRAINT merkle_tree_metadata_pkey PRIMARY KEY (id);


--
-- Name: payout_logs payout_logs_pkey; Type: CONSTRAINT; Schema: public; Owner: twzrd
--

ALTER TABLE ONLY public.payout_logs
    ADD CONSTRAINT payout_logs_pkey PRIMARY KEY (id);


--
-- Name: processed_payouts processed_payouts_pkey; Type: CONSTRAINT; Schema: public; Owner: twzrd
--

ALTER TABLE ONLY public.processed_payouts
    ADD CONSTRAINT processed_payouts_pkey PRIMARY KEY (idempotency_key);


--
-- Name: reward_epochs reward_epochs_pkey; Type: CONSTRAINT; Schema: public; Owner: twzrd
--

ALTER TABLE ONLY public.reward_epochs
    ADD CONSTRAINT reward_epochs_pkey PRIMARY KEY (epoch_id);


--
-- Name: sealed_epochs sealed_epochs_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.sealed_epochs
    ADD CONSTRAINT sealed_epochs_pkey PRIMARY KEY (epoch, channel, token_group, category);


--
-- Name: sealed_participants sealed_participants_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.sealed_participants
    ADD CONSTRAINT sealed_participants_pkey PRIMARY KEY (epoch, channel, idx);


--
-- Name: social_verification social_verification_pkey; Type: CONSTRAINT; Schema: public; Owner: twzrd
--

ALTER TABLE ONLY public.social_verification
    ADD CONSTRAINT social_verification_pkey PRIMARY KEY (wallet);


--
-- Name: streams streams_pkey; Type: CONSTRAINT; Schema: public; Owner: twzrd
--

ALTER TABLE ONLY public.streams
    ADD CONSTRAINT streams_pkey PRIMARY KEY (stream_id);


--
-- Name: submissions submissions_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.submissions
    ADD CONSTRAINT submissions_pkey PRIMARY KEY (id);


--
-- Name: submissions submissions_user_pubkey_channel_epoch_key; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.submissions
    ADD CONSTRAINT submissions_user_pubkey_channel_epoch_key UNIQUE (user_pubkey, channel, epoch);


--
-- Name: suppression_list suppression_list_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.suppression_list
    ADD CONSTRAINT suppression_list_pkey PRIMARY KEY (user_hash);


--
-- Name: suppression_log suppression_log_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.suppression_log
    ADD CONSTRAINT suppression_log_pkey PRIMARY KEY (id);


--
-- Name: system_config system_config_pkey; Type: CONSTRAINT; Schema: public; Owner: twzrd
--

ALTER TABLE ONLY public.system_config
    ADD CONSTRAINT system_config_pkey PRIMARY KEY (key);


--
-- Name: twitch_events twitch_events_pkey; Type: CONSTRAINT; Schema: public; Owner: twzrd
--

ALTER TABLE ONLY public.twitch_events
    ADD CONSTRAINT twitch_events_pkey PRIMARY KEY (message_id);


--
-- Name: twitch_tokens twitch_tokens_pkey; Type: CONSTRAINT; Schema: public; Owner: twzrd
--

ALTER TABLE ONLY public.twitch_tokens
    ADD CONSTRAINT twitch_tokens_pkey PRIMARY KEY (user_id);


--
-- Name: twitch_wallet_bindings twitch_wallet_bindings_pkey; Type: CONSTRAINT; Schema: public; Owner: twzrd
--

ALTER TABLE ONLY public.twitch_wallet_bindings
    ADD CONSTRAINT twitch_wallet_bindings_pkey PRIMARY KEY (twitch_id);


--
-- Name: user_mapping user_mapping_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_mapping
    ADD CONSTRAINT user_mapping_pkey PRIMARY KEY (user_hash);


--
-- Name: user_signals user_signals_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.user_signals
    ADD CONSTRAINT user_signals_pkey PRIMARY KEY (epoch, channel, user_hash, signal_type, "timestamp");


--
-- Name: user_wallets user_wallets_pkey; Type: CONSTRAINT; Schema: public; Owner: twzrd
--

ALTER TABLE ONLY public.user_wallets
    ADD CONSTRAINT user_wallets_pkey PRIMARY KEY (twitch_user_id);


--
-- Name: viewer_activity viewer_activity_pkey; Type: CONSTRAINT; Schema: public; Owner: twzrd
--

ALTER TABLE ONLY public.viewer_activity
    ADD CONSTRAINT viewer_activity_pkey PRIMARY KEY (viewer_id, stream_id);


--
-- Name: viewer_snapshots viewer_snapshots_pkey; Type: CONSTRAINT; Schema: public; Owner: postgres
--

ALTER TABLE ONLY public.viewer_snapshots
    ADD CONSTRAINT viewer_snapshots_pkey PRIMARY KEY (id);


--
-- Name: weighted_participants weighted_participants_pkey; Type: CONSTRAINT; Schema: public; Owner: twzrd
--

ALTER TABLE ONLY public.weighted_participants
    ADD CONSTRAINT weighted_participants_pkey PRIMARY KEY (channel, epoch, user_hash);


--
-- Name: idx_channel_participation_channel; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_channel_participation_channel ON public.channel_participation USING btree (channel);


--
-- Name: idx_channel_participation_epoch; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_channel_participation_epoch ON public.channel_participation USING btree (epoch);


--
-- Name: idx_channel_participation_token_channel; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_channel_participation_token_channel ON public.channel_participation USING btree (token_group, channel, epoch);


--
-- Name: idx_channel_participation_token_group; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_channel_participation_token_group ON public.channel_participation USING btree (token_group, epoch);


--
-- Name: idx_cls_claims_epoch; Type: INDEX; Schema: public; Owner: twzrd
--

CREATE INDEX idx_cls_claims_epoch ON public.cls_claims USING btree (epoch_id);


--
-- Name: idx_cls_claims_signature; Type: INDEX; Schema: public; Owner: twzrd
--

CREATE INDEX idx_cls_claims_signature ON public.cls_claims USING btree (tx_signature);


--
-- Name: idx_cls_claims_status; Type: INDEX; Schema: public; Owner: twzrd
--

CREATE INDEX idx_cls_claims_status ON public.cls_claims USING btree (tx_status);


--
-- Name: idx_cls_claims_wallet; Type: INDEX; Schema: public; Owner: twzrd
--

CREATE INDEX idx_cls_claims_wallet ON public.cls_claims USING btree (wallet);


--
-- Name: idx_cls_discovered_channels_category; Type: INDEX; Schema: public; Owner: twzrd
--

CREATE INDEX idx_cls_discovered_channels_category ON public.cls_discovered_channels USING btree (category, channel_name);


--
-- Name: idx_cls_discovered_channels_channel; Type: INDEX; Schema: public; Owner: twzrd
--

CREATE INDEX idx_cls_discovered_channels_channel ON public.cls_discovered_channels USING btree (channel_name, discovered_at DESC);


--
-- Name: idx_cls_discovered_channels_run; Type: INDEX; Schema: public; Owner: twzrd
--

CREATE INDEX idx_cls_discovered_channels_run ON public.cls_discovered_channels USING btree (discovery_run_id);


--
-- Name: idx_cls_discovered_channels_timestamp; Type: INDEX; Schema: public; Owner: twzrd
--

CREATE INDEX idx_cls_discovered_channels_timestamp ON public.cls_discovered_channels USING btree (discovered_at DESC);


--
-- Name: idx_epochs_finalized; Type: INDEX; Schema: public; Owner: twzrd
--

CREATE INDEX idx_epochs_finalized ON public.reward_epochs USING btree (finalized);


--
-- Name: idx_epochs_stream; Type: INDEX; Schema: public; Owner: twzrd
--

CREATE INDEX idx_epochs_stream ON public.reward_epochs USING btree (stream_id);


--
-- Name: idx_merkle_roots_channel; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_merkle_roots_channel ON public.merkle_roots USING btree (channel);


--
-- Name: idx_merkle_roots_epoch; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_merkle_roots_epoch ON public.merkle_roots USING btree (epoch DESC);


--
-- Name: idx_merkle_roots_sealed; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_merkle_roots_sealed ON public.merkle_roots USING btree (sealed, created_at);


--
-- Name: idx_participation_category; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_participation_category ON public.channel_participation USING btree (category, token_group, epoch);


--
-- Name: idx_payout_logs_status; Type: INDEX; Schema: public; Owner: twzrd
--

CREATE INDEX idx_payout_logs_status ON public.payout_logs USING btree (status);


--
-- Name: idx_payout_logs_viewer; Type: INDEX; Schema: public; Owner: twzrd
--

CREATE INDEX idx_payout_logs_viewer ON public.payout_logs USING btree (viewer_id);


--
-- Name: idx_payout_logs_wallet; Type: INDEX; Schema: public; Owner: twzrd
--

CREATE INDEX idx_payout_logs_wallet ON public.payout_logs USING btree (wallet);


--
-- Name: idx_payouts_epoch; Type: INDEX; Schema: public; Owner: twzrd
--

CREATE INDEX idx_payouts_epoch ON public.processed_payouts USING btree (epoch_id);


--
-- Name: idx_sealed_epochs_category; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_sealed_epochs_category ON public.sealed_epochs USING btree (category, token_group, epoch);


--
-- Name: idx_sealed_epochs_epoch; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_sealed_epochs_epoch ON public.sealed_epochs USING btree (epoch DESC);


--
-- Name: idx_sealed_epochs_published; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_sealed_epochs_published ON public.sealed_epochs USING btree (published);


--
-- Name: idx_sealed_epochs_token_group; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_sealed_epochs_token_group ON public.sealed_epochs USING btree (token_group, epoch, channel);


--
-- Name: idx_sealed_participants_category; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_sealed_participants_category ON public.sealed_participants USING btree (category, token_group, epoch, channel);


--
-- Name: idx_sealed_participants_lookup; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_sealed_participants_lookup ON public.sealed_participants USING btree (epoch, channel);


--
-- Name: idx_sealed_participants_token_group; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_sealed_participants_token_group ON public.sealed_participants USING btree (token_group, epoch, channel);


--
-- Name: idx_snapshots_minute; Type: INDEX; Schema: public; Owner: twzrd
--

CREATE INDEX idx_snapshots_minute ON public.chatter_snapshots USING btree (minute_ts);


--
-- Name: idx_snapshots_stream; Type: INDEX; Schema: public; Owner: twzrd
--

CREATE INDEX idx_snapshots_stream ON public.chatter_snapshots USING btree (stream_id);


--
-- Name: idx_social_verification_wallet; Type: INDEX; Schema: public; Owner: twzrd
--

CREATE INDEX idx_social_verification_wallet ON public.social_verification USING btree (wallet);


--
-- Name: idx_streams_broadcaster; Type: INDEX; Schema: public; Owner: twzrd
--

CREATE INDEX idx_streams_broadcaster ON public.streams USING btree (broadcaster_id);


--
-- Name: idx_streams_processed; Type: INDEX; Schema: public; Owner: twzrd
--

CREATE INDEX idx_streams_processed ON public.streams USING btree (processed);


--
-- Name: idx_streams_status; Type: INDEX; Schema: public; Owner: twzrd
--

CREATE INDEX idx_streams_status ON public.streams USING btree (status);


--
-- Name: idx_submissions_channel_epoch; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_submissions_channel_epoch ON public.submissions USING btree (channel, epoch);


--
-- Name: idx_submissions_created; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_submissions_created ON public.submissions USING btree (created_at DESC);


--
-- Name: idx_submissions_user; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_submissions_user ON public.submissions USING btree (user_pubkey);


--
-- Name: idx_suppression_username; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_suppression_username ON public.suppression_list USING btree (lower(username));


--
-- Name: idx_user_signals_epoch; Type: INDEX; Schema: public; Owner: postgres
--

CREATE INDEX idx_user_signals_epoch ON public.user_signals USING btree (epoch);


--
-- Name: idx_wallet_unique; Type: INDEX; Schema: public; Owner: twzrd
--

CREATE UNIQUE INDEX idx_wallet_unique ON public.twitch_wallet_bindings USING btree (wallet);


--
-- Name: idx_wallets_address; Type: INDEX; Schema: public; Owner: twzrd
--

CREATE INDEX idx_wallets_address ON public.user_wallets USING btree (wallet_address);


--
-- Name: SCHEMA public; Type: ACL; Schema: -; Owner: postgres
--

GRANT USAGE ON SCHEMA public TO twzrd;


--
-- Name: TABLE attention_index; Type: ACL; Schema: public; Owner: postgres
--

GRANT ALL ON TABLE public.attention_index TO twzrd;


--
-- Name: TABLE channel_participation; Type: ACL; Schema: public; Owner: postgres
--

GRANT ALL ON TABLE public.channel_participation TO twzrd;


--
-- Name: TABLE channel_payouts; Type: ACL; Schema: public; Owner: postgres
--

GRANT ALL ON TABLE public.channel_payouts TO twzrd;


--
-- Name: TABLE l2_tree_cache; Type: ACL; Schema: public; Owner: postgres
--

GRANT ALL ON TABLE public.l2_tree_cache TO twzrd;


--
-- Name: TABLE merkle_roots; Type: ACL; Schema: public; Owner: postgres
--

GRANT ALL ON TABLE public.merkle_roots TO twzrd;


--
-- Name: SEQUENCE merkle_roots_id_seq; Type: ACL; Schema: public; Owner: postgres
--

GRANT ALL ON SEQUENCE public.merkle_roots_id_seq TO twzrd;


--
-- Name: TABLE merkle_tree_metadata; Type: ACL; Schema: public; Owner: postgres
--

GRANT ALL ON TABLE public.merkle_tree_metadata TO twzrd;


--
-- Name: SEQUENCE merkle_tree_metadata_id_seq; Type: ACL; Schema: public; Owner: postgres
--

GRANT ALL ON SEQUENCE public.merkle_tree_metadata_id_seq TO twzrd;


--
-- Name: TABLE sealed_epochs; Type: ACL; Schema: public; Owner: postgres
--

GRANT ALL ON TABLE public.sealed_epochs TO twzrd;


--
-- Name: TABLE sealed_participants; Type: ACL; Schema: public; Owner: postgres
--

GRANT ALL ON TABLE public.sealed_participants TO twzrd;


--
-- Name: TABLE submissions; Type: ACL; Schema: public; Owner: postgres
--

GRANT ALL ON TABLE public.submissions TO twzrd;


--
-- Name: SEQUENCE submissions_id_seq; Type: ACL; Schema: public; Owner: postgres
--

GRANT ALL ON SEQUENCE public.submissions_id_seq TO twzrd;


--
-- Name: TABLE suppression_list; Type: ACL; Schema: public; Owner: postgres
--

GRANT ALL ON TABLE public.suppression_list TO twzrd;


--
-- Name: TABLE suppression_log; Type: ACL; Schema: public; Owner: postgres
--

GRANT ALL ON TABLE public.suppression_log TO twzrd;


--
-- Name: SEQUENCE suppression_log_id_seq; Type: ACL; Schema: public; Owner: postgres
--

GRANT ALL ON SEQUENCE public.suppression_log_id_seq TO twzrd;


--
-- Name: TABLE user_mapping; Type: ACL; Schema: public; Owner: postgres
--

GRANT ALL ON TABLE public.user_mapping TO twzrd;


--
-- Name: TABLE user_signals; Type: ACL; Schema: public; Owner: postgres
--

GRANT ALL ON TABLE public.user_signals TO twzrd;


--
-- Name: TABLE viewer_snapshots; Type: ACL; Schema: public; Owner: postgres
--

GRANT ALL ON TABLE public.viewer_snapshots TO twzrd;


--
-- Name: SEQUENCE viewer_snapshots_id_seq; Type: ACL; Schema: public; Owner: postgres
--

GRANT ALL ON SEQUENCE public.viewer_snapshots_id_seq TO twzrd;


--
-- Name: DEFAULT PRIVILEGES FOR SEQUENCES; Type: DEFAULT ACL; Schema: public; Owner: postgres
--

ALTER DEFAULT PRIVILEGES FOR ROLE postgres IN SCHEMA public GRANT ALL ON SEQUENCES  TO twzrd;


--
-- Name: DEFAULT PRIVILEGES FOR TABLES; Type: DEFAULT ACL; Schema: public; Owner: postgres
--

ALTER DEFAULT PRIVILEGES FOR ROLE postgres IN SCHEMA public GRANT ALL ON TABLES  TO twzrd;


--
-- PostgreSQL database dump complete
--

\unrestrict 84K8KKI21DWmztjRdNAy3As2c0zumJJGhrnI5qz5JdQqQgPAUxafoaeW6jJVIs3

