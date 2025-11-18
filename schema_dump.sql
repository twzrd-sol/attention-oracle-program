--
-- PostgreSQL database dump
--

\restrict EGXsnQadpJ8xhUOlSCZmWJOn5SMaEaBvnj76EmekAfXy2H0XGh3pdVuhEI1aHFd

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

SET default_tablespace = '';

SET default_table_access_method = heap;

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
-- Name: TABLE merkle_roots; Type: ACL; Schema: public; Owner: postgres
--

GRANT ALL ON TABLE public.merkle_roots TO twzrd;


--
-- Name: TABLE merkle_tree_metadata; Type: ACL; Schema: public; Owner: postgres
--

GRANT ALL ON TABLE public.merkle_tree_metadata TO twzrd;


--
-- PostgreSQL database dump complete
--

\unrestrict EGXsnQadpJ8xhUOlSCZmWJOn5SMaEaBvnj76EmekAfXy2H0XGh3pdVuhEI1aHFd

