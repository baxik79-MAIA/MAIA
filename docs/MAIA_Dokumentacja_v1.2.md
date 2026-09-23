---
title: "MAIA - Multichannel Automation & Intelligent Assistance"
subtitle: "Dokumentacja Techniczna i Architektura Wykonawcza v1.2"
author: "MAIA Architecture"
date: "11.09.2026"
lang: pl-PL
---

# Spis treści

- 0. Streszczenie wykonawcze
- 1. Ewolucja projektu i granica między prototypem a produktem
- 2. Product Charter
- 3. Tryby działania
- 4. Referencyjny UX
- 5. Architektura systemowa
- 6. Canonical Spec i Spec Guard
- 7. Model domenowy i proweniencja
- 8. Maszyny stanów
- 9. Agent Orchestrator
- 10. Policy Router, Privacy Gate i Approval Gate
- 11. Model Router i klucze API
- 12. Connector Framework
- 13. Outlook Classic / TrustedBridge Connector
- 14. Microsoft Graph Connector
- 15. Teams i Microsoft 365 Agent Surface
- 16. Mail Intelligence
- 17. Calendar & Meeting Intelligence
- 18. WorkGraph
- 19. Commitment Engine
- 20. Relationship Intelligence
- 21. Memory Engine
- 22. Attachment, Search i RAG Pipeline
- 23. Scheduler i proaktywność
- 24. MCP Gateway
- 25. Tool Sandbox i Skill Runtime
- 26. Credential Vault i rotacja API Keys
- 27. Permission Model i Governance
- 28. Persistence
- 29. Synchronizacja i zdarzenia
- 30. Notifications i Approval UX
- 31. Error Taxonomy, retry i idempotency
- 32. Security Threat Model
- 33. Klasyfikacja danych i prywatność
- 34. Audit, observability i provenance
- 35. Internacjonalizacja, accessibility i design system
- 36. Export, backup, portability i migration
- 37. Aktualizacje i deployment
- 38. Repozytorium i granice modułów
- 39. Coding Standards
- 40. Test Strategy
- 41. Acceptance Criteria
- 42. Roadmap implementacyjny - pionowe slice-y
- 43. AGENTS.md - zasady dla modeli kodujących
- 44. Runbooki operacyjne
- 45. ADR Register
- 46. Viktor parity vs MAIA advantage
- 47. Strategia Microsoft 365 i migracji Outlook
- 48. Zewnętrzne źródła - snapshot 11.09.2026
- 49. Otwarte decyzje przed rozpoczęciem pełnego kodowania
- ZAŁĄCZNIK A - Canonical spec/*.yaml
- ZAŁĄCZNIK B - ADR-y
- ZAŁĄCZNIK C - Runbooki
- ZAŁĄCZNIK D - Implementation Pack manifest

```{=openxml}
<w:p><w:r><w:br w:type="page"/></w:r></w:p>
```

**Platforma referencyjna Alpha:** Windows desktop, ponieważ bootstrap Outlook Classic/COM jest zależny od Windows. **MAIA Core jest kontraktowo OS-neutralny**; zależności COM/Win32 są dozwolone wyłącznie za granicą adaptera/surface. Docelowe powierzchnie obejmują Teams, Outlook/Graph, MCP i inne kanały bez wymogu Windows.  
**Reguła normatywności:** jedynym źródłem normatywnych kontraktów są pliki `spec/*.yaml` w dołączonym Implementation Pack. Rejestry enumów, stanów, polityk, acceptance i podobne fragmenty master dokumentu są **generowane z YAML**, a nie utrzymywane ręcznie. Dokument narracyjny może wyjaśniać intencję, ale nie tworzy konkurencyjnego kontraktu.

![Architektura referencyjna MAIA](MAIA_v1_2_Implementation_Pack/assets/architecture.png){ width=90% }

## 0. Streszczenie wykonawcze

MAIA - **Multichannel Automation & Intelligent Assistance** - nie jest dodatkiem do Outlooka i nie jest chatbotem odpowiadającym na pojedyncze pytania. Rozwinięcie akronimu zostało w v1.1 zmienione z historycznego „Mail Automation” na „Multichannel Automation”, aby nazwa produktu odpowiadała channel-neutralnej architekturze; Mail Intelligence pozostaje jednym z jego strategicznych wyróżników. Jest warstwą operacyjną nad komunikacją i pracą użytkownika: odbiera intencję, buduje plan, dobiera model i connector, kontroluje ryzyko, wykonuje dozwolone działania, zapisuje proweniencję i aktualizuje długotrwały kontekst pracy. Poczta jest jednym z najważniejszych źródeł i kanałów działania, ale pozostaje wymiennym connector-em.

Obecna ścieżka Outlook Classic + TrustedBridge zostaje zachowana jako `OutlookClassicConnector`. Dzięki temu prototyp działający w ograniczonym środowisku firmowym nie jest wyrzucany. Jednocześnie MAIA Core nie zależy od COM. Gdy organizacja przejdzie na nowy Outlook, connector może zostać zastąpiony Microsoft Graph i Outlook Web Add-in bez migracji całej inteligencji, pamięci, WorkGraphu i historii audytowej.

Najważniejszą przewagą MAIA ma być głębokość rozumienia komunikacji: powiązanie ludzi, projektów, maili, spotkań, decyzji, zobowiązań i plików w jeden WorkGraph. MAIA nie tylko streszcza wiadomość; potrafi rozpoznać, że mail zmienia wcześniejsze ustalenie, że odpowiedź jest oczekiwana, że ktoś obiecał dostarczyć element do określonego terminu i że ta obietnica wpływa na następne spotkanie.

**v1.2 Topology & Concurrency Hardening:** v1.1 zamknęła pierwszą serię driftów kontraktowych; v1.2 zamyka luki topologii i wykonania: Teams vs lokalny store, quiescent pause, RFC 8785 dla MCP fingerprints, stale approvals, bezpieczne renderowanie Approval Cards, COM STA/busy handling, source-freshness przed side effectem, `outcome_unknown`/reconciliation, jawnie nieautorytatywne inference w WorkGraphu oraz privacy-safe tombstoning. `tools/validate_spec.py` wykonuje także referencyjny test JCS.

### 0.1. Cele jakościowe v1

**Uniwersalność**

- **Cel:** Outlook Classic dzisiaj, M365/Graph/Teams jutro, inne ekosystemy później.

- **Mechanizm:** Connector contract + capability registry.



**Bezpieczeństwo wykonawcze**

- **Cel:** Agent może działać, ale materialne skutki podlegają politykom i zatwierdzeniom.

- **Mechanizm:** ApprovalGate, payload hash, audit trail.



**Pamięć pracy, nie czatów**

- **Cel:** Kontekst ma przetrwać zmianę kanału i klienta.

- **Mechanizm:** WorkGraph + Memory Claims + source provenance.



**Brak vendor lock-in**

- **Cel:** Modele i klucze API wymienia się bez zmiany logiki biznesowej.

- **Mechanizm:** Model Router + dynamic provider registry + vault.



**Gotowość implementacyjna**

- **Cel:** Agent kodujący nie zgaduje stanów i kontraktów.

- **Mechanizm:** Canonical YAML + ADR + Spec Guard + acceptance vectors.



**Enterprise-ready**

- **Cel:** Rozwiązanie ma respektować tenant, scope, audyt i centralne polityki.

- **Mechanizm:** Least privilege, Entra/Graph profiles, stricter-admin-policy rule.



### 0.2. Zasady niepodlegające negocjacji

- Outlook Classic COM jest connector-em bootstrapowym, a nie fundamentem produktu.
- Użytkownik pozostaje najwyższą instancją dla materialnych działań; tenant/admin może narzucić wyłącznie bardziej restrykcyjne ograniczenia.
- Wysłanie wiadomości, operacja destrukcyjna lub uprzywilejowana nie może wydarzyć się po cichu w polityce domyślnej.
- MAIA zapisuje requested/actual connector i requested/actual model dla każdej próby wykonania.
- Sekrety nie trafiają do SQLite, logów, eksportów, bundle ani promptów.
- Mail, Teams, dokumenty i wyniki zewnętrznych narzędzi są treścią niezaufaną; nie mogą samodzielnie nadawać uprawnień ani sterować narzędziami.
- Każde zobowiązanie i decyzja pochodzące z komunikacji zachowują źródło i poziom pewności.
- Zmiana kontraktu zaczyna się w `spec/*.yaml`; generated artifacts nie są edytowane ręcznie.
- Nowy kanał, model lub connector nie może wymagać przepisywania MAIA Core.

## 1. Ewolucja projektu i granica między prototypem a produktem

### 1.1. Bootstrap: Outlook Classic + TrustedBridge

Pierwszy działający wariant MAIA powstał wokół Outlook Classic, lokalnego mechanizmu pośredniczącego TrustedBridge i lokalnego modelu. Było to właściwe rozwiązanie dla środowiska, w którym klasyczny dostęp API może być niedostępny lub administracyjnie ograniczony. Ten wariant pozostaje ważnym trybem produktu: pozwala rozwijać inteligencję MAIA zanim organizacja udostępni Graph lub pełną rejestrację aplikacji.

Błąd, którego v1 nie może popełnić, polegałby na przeniesieniu logiki biznesowej do skryptów COM. Od tego momentu wszystkie operacje Outlook Classic są normalizowane przez connector; mail, osoba, wątek, draft i akcja wysłania mają ten sam model domenowy niezależnie od źródła.

### 1.2. Dlaczego architektura musi odejść od zależności od COM

Microsoft dokumentuje, że nowy Outlook na Windows nie obsługuje dodatków COM/VSTO i kieruje rozszerzenia w stronę Outlook Web Add-ins. To sprawia, że COM jest użytecznym adapterem przejściowym, ale nie bezpiecznym założeniem wieloletniego produktu [MS-03].

### 1.3. Kierunek docelowy

Docelowo MAIA ma działać równocześnie w desktopowym Command Center, w Teams i kontekstowo w Outlooku. Microsoft 365 Agents SDK zapewnia warstwę kanałową niezależną od logiki AI, a Teams może być pierwszoplanowym interfejsem konwersacyjnym [MS-01, MS-02]. Dane M365 są pozyskiwane przez Graph, zaś interfejs Outlook Add-in daje kontekst i kontrolki bez robienia z klienta pocztowego serwera aplikacji.

![Migracja connectorów bez migracji rdzenia](MAIA_v1_2_Implementation_Pack/assets/migration.png){ width=90% }

## 2. Product Charter

### 2.1. Problem

Praca menedżerska i projektowa jest rozproszona. Ustalenie zaczyna się na spotkaniu, zostaje doprecyzowane mailem, pojawia się w Teams, a załącznik trafia na SharePoint lub dysk. Zwykły chatbot widzi tylko aktualny prompt; zwykły klient pocztowy widzi tylko wiadomości. MAIA ma utrzymywać spójny model tego, co dzieje się w pracy użytkownika i pomagać przejść od informacji do bezpiecznego działania.

### 2.2. Obietnica produktu

**Mail Intelligence. Human Simplicity.** MAIA ma rozumieć kontekst, pilnować zobowiązań, przygotowywać komunikację i wykonywać rutynowe działania, ale zawsze w granicach jawnej polityki, uprawnień i proweniencji.

### 2.3. Główne cele

- jedno miejsce do rozmowy z agentem niezależnie od kanału
- głęboka integracja z pocztą i kalendarzem
- Teams jako pełnoprawny interfejs
- WorkGraph ludzi/projektów/decyzji/zobowiązań
- proaktywne briefingi i follow-upy
- wymienne modele i credential profiles
- MCP do szybkiej rozbudowy narzędzi
- lokalny/legacy tryb dla środowisk ograniczonych
- enterprise policy + pełny audit

### 2.4. Non-goals

- obejście polityk bezpieczeństwa organizacji
- scraping konsumenckich interfejsów AI
- przechowywanie haseł/kluczy w projekcie
- automatyczne wysyłanie zewnętrznych maili bez polityki/zgody
- profilowanie prywatnych cech współpracowników
- uzależnienie całego produktu od jednego modelu, jednego mail clienta lub jednego tenantu

## 3. Tryby działania

**local_legacy**

- **external_api_required:** False

- **mail_transport:** outlook_classic_com

- **llm:** local_or_manual

- **egress:** policy_dependent

- **purpose:** Current bootstrap mode for locked-down Windows environments.



**connected_m365**

- **external_api_required:** True

- **mail_transport:** microsoft_graph

- **channels:** ['teams', 'outlook', 'm365_copilot']

- **egress:** tenant_and_policy_controlled



**connected_generic**

- **external_api_required:** True

- **mail_transport:** connector_defined

- **channels:** ['desktop', 'web', 'third_party']

- **egress:** connector_and_policy_controlled



**hybrid**

- **external_api_required:** optional

- **mail_transport:** best_available_connector

- **llm:** local_and_cloud

- **egress:** per_task_policy



**policy_auto**

- **external_api_required:** depends

- **mail_transport:** routed

- **llm:** routed

- **egress:** depends

- **purpose:** Select only among policy-compliant connectors and models.



Tryb jest właściwością planu/zadania, a nie całej aplikacji na zawsze. Użytkownik może pracować z pocztą przez Graph, ale analizować wrażliwy załącznik lokalnym modelem; router musi potrafić rozdzielić ścieżkę danych i ścieżkę inferencji.

## 4. Referencyjny UX

MAIA powinna mieć jeden spójny model zadania widoczny na różnych powierzchniach. Rozmowa rozpoczęta w Teams może być kontynuowana w desktopie; approval wykonany w Teams aktualizuje to samo `Approval`, a wynik wysłania przez Graph pojawia się w audycie niezależnie od kanału.

**command_center**

- **Elementy:** today_brief, priority_inbox, commitments, meetings, awaiting_approvals



**conversation**

- **Elementy:** chat_with_maia, task_trace, sources, approval_cards



**mail**

- **Elementy:** thread_summary, why_it_matters, draft, commitments, relationship_context



**workgraph**

- **Elementy:** people, projects, threads, decisions, commitments, timeline



**settings**

- **Elementy:** connectors, models, credentials, policies, memory, scheduler, mcp, audit



### 4.1. Command Center - ekran startowy

Ekran „Dzisiaj” nie jest skrzynką odbiorczą. Jest pulpitowym filtrem pracy: priorytetowe wątki, spotkania z briefingiem, zobowiązania bliskie terminu, oczekiwane odpowiedzi, działania czekające na zgodę oraz anomalie typu sprzeczne ustalenia. Użytkownik może wejść do źródła, ale nie musi ręcznie przeglądać wszystkich kanałów.

### 4.2. Approval Card

Każda zgoda pokazuje dokładny efekt: adresatów, kanał, treść lub różnicę względem draftu, załączniki, klasyfikację danych, connector, model wykorzystany do przygotowania, ryzyko i powód wymaganej zgody. Zatwierdzenie dotyczy hasha akcji; zmiana treści lub odbiorców unieważnia zgodę.

## 5. Architektura systemowa

MAIA ma jeden logiczny Core, ale nie jeden proces ani jedną topologię. Kluczowym wymaganiem v1.2 jest **single state authority per workspace**: kanał Teams, desktop i lokalny sidecar nie mogą stać się niezależnymi „mózgami” z własną wersją planu, Approval i WorkGraphu.

![Architektura MAIA v1.2](MAIA_v1_2_Implementation_Pack/assets/architecture.png){ width=94% }

### 5.1. Surfaces i Channel Adapter

Desktop, Outlook Add-in, Teams oraz API/MCP clients są surfaces. Teams w produkcji dochodzi do zarządzanego publicznego HTTPS endpointu Channel Adaptera. Channel Adapter odpowiada za transport, tożsamość kanału i delivery envelope - nie za planowanie i nie za obchodzenie ApprovalGate.

### 5.2. Dwie legalne topologie stanu

- **Local-authoritative:** Core i SQLite są na urządzeniu. Teams - jeśli włączony - dociera przez zarządzany Channel Adapter i outbound-only relay. Chmura nie otwiera pliku SQLite.
- **Managed enterprise:** headless Core i autorytatywny transactional store działają jako usługa; PostgreSQL jest profilem referencyjnym. Desktop jest thin clientem lub lokalnym capability workerem.

Tryb hybrid może łączyć managed Core z lokalnym workerem (np. Outlook Classic), ale nie może stworzyć multi-master. Szczegóły transportu są normatywne w `spec/transport.yaml` i deployment profile.

### 5.3. Core intelligence

`Planner/Executor`, Policy/Approval, WorkGraph, Mail Intelligence, Commitment/Relationship/Memory, Scheduler i Model Router pozostają channel-neutral i OS-neutral. Connector/surface może mieć zależności platformowe, ale nie przenosi logiki biznesowej do klienta Microsoft/COM.

## 6. Canonical Spec i Spec Guard

MAIA przejmuje najważniejszą lekcję AI Round Table v6: kontrakt ma istnieć raz. `spec/*.yaml` definiuje enumy, stany, ryzyko, capabilities, storage, IPC, compliance i acceptance. ADR wyjaśnia dlaczego. Dokumentacja narracyjna jest artefaktem generowanym/wyjaśniającym i nie może być równoległym źródłem prawdy.

**Hierarchia źródeł**

- **1:** `spec/*.yaml` - wymagania normatywne.
- **2:** `docs/adr/*.md` - decyzje architektoniczne.
- **3:** `prompts/**`, `locales/**` - assety treści.
- **4:** `generated/**` - artefakty generowane; nigdy ręcznie.
- **5:** Master Markdown/DOCX/PDF - generowana narracja/rejestry + wyjaśnienia.
- **6:** kod - implementacja o najniższym autorytecie przy konflikcie.

### 6.1. Wykonywalny Spec Guard

v1.0 opisywała Spec Guard, ale nie dostarczała kompletnego walidatora kontraktowego. v1.1 naprawia to: `tools/validate_spec.py` parsuje wszystkie canonical YAML i odrzuca co najmniej: niepoprawne `schema_version`, `risk` spoza `RiskClass`, nieistniejący dynamiczny risk classifier, output classifiera spoza enuma, niepełne mapowanie gate -> `ApprovalState`, implicit/stringowe FSM transitions i duplikaty acceptance ID.

Aktualny pack przechodzi tę bramkę jako **23 poprawnie sparsowane pliki YAML + contract checks**. Docelowy CI rozszerza ją o JSON Schema, generated drift, referencje IPC/encje, migrations i testy kontraktowe.

### 6.2. Brak ręcznego duplikowania rejestrów

Listy ryzyk, stanów i acceptance widoczne w Master są renderowane z YAML. `generated/canonical_registry.md` jest prostym, audytowalnym przykładem tej zasady. Zmiana canonical spec bez regeneracji dokumentacji ma być błędem builda; `docs_updated` przestaje być wyłącznie obowiązkiem pamięci developera.

## 7. Model domenowy i proweniencja

Trzy rozdzielenia są kluczowe: **Task != Plan != Action != Run**, **ConnectorProfile != CredentialProfile**, oraz **Person/Project facts != MemoryClaim inference**. Dzięki temu retry nie duplikuje zadania, wymiana credentiala nie zmienia connectora, a hipoteza o projekcie nie staje się faktem tylko dlatego, że model brzmi pewnie.

**Workspace**

- **Pola:** id, name, owner_id, policy_profile_id, created_at, updated_at



**Person**

- **Pola:** id, display_name, emails, teams_ids, organization_id, relationship_profile_id, confidence, created_at, updated_at



**Organization**

- **Pola:** id, name, domains, tags, created_at, updated_at



**Project**

- **Pola:** id, name, status, tags, summary, owner_id, created_at, updated_at



**Thread**

- **Pola:** id, channel, external_thread_id, subject, project_id, participants, last_activity_at, classification



**Message**

- **Pola:** id, thread_id, external_id, sender_id, recipients, sent_at, received_at, body_ref, trust, classification



**Meeting**

- **Pola:** id, external_id, title, start_at, end_at, participants, project_id, transcript_ref, status



**Commitment**

- **Pola:** id, owner_person_id, beneficiary_person_id, project_id, source_ref, text, due_at, status, confidence



**Decision**

- **Pola:** id, project_id, source_ref, statement, decided_by, decided_at, confidence, supersedes_id



**WorkItem**

- **Pola:** id, project_id, title, description, owner_id, due_at, status, priority, source_ref



**RelationshipProfile**

- **Pola:** id, person_id, language, tone, formality, response_style, working_context, last_contact_at



**MemoryClaim**

- **Pola:** id, scope, subject_ref, predicate, value_json, source_ref, status, confidence, valid_from, valid_until



**AgentTask**

- **Pola:** id, workspace_id, request_text, origin_surface, origin_ref, state, privacy_class, requested_by, created_at



**ExecutionPlan**

- **Pola:** id, task_id, version, risk_summary, estimated_cost, approval_requirement, created_at



**Action**

- **Pola:** id, plan_id, ordinal, action_type, connector_profile_id, risk_class, state, input_ref, result_ref



**Run**

- **Pola:** id, action_id, attempt, requested_connector_id, actual_connector_id, requested_model_id, actual_model_id, state, started_at, ended_at



**Approval**

- **Pola:** id, task_id, action_id, policy_id, state, requested_at, decided_at, decided_by, decision_note, action_hash, version, expires_at, origin_surface, decided_surface



**ConnectorProfile**

- **Pola:** id, connector_type, label, capabilities, credential_profile_id, health, policy_tags



**CredentialProfile**

- **Pola:** id, provider, label, secret_ref, scopes, status, last_validated_at



**ModelProfile**

- **Pola:** id, provider, model_id, endpoint_profile_id, credential_profile_id, capabilities, privacy_tags, billing_mode



**ScheduledJob**

- **Pola:** id, workspace_id, task_template_ref, schedule, state, next_run_at, last_run_at



**Artifact**

- **Pola:** id, kind, name, path_or_external_ref, sha256, trust, classification, created_at



**Event**

- **Pola:** id, task_id, run_id, event_type, payload_json, created_at



**AuditRecord**

- **Pola:** id, actor, operation, target_ref, risk_class, decision, hash_chain_prev, created_at



## 8. Maszyny stanów

Dokładne listy i przejścia są normatywnie utrzymywane wyłącznie w `spec/states.yaml`. v1.2 rozdziela **pauzę schedulerową** od preempcji akcji: `ActionState.paused` celowo nie istnieje. Task przechodzi `running -> pausing -> paused`; dopiero po opróżnieniu in-flight Runów stan `paused` jest prawdziwy. Dodano też `RunState` oraz ścieżkę `outcome_unknown -> reconciling`, aby nie wykonywać ślepego retry po niejednoznacznym side-effekcie.

**AgentTaskState**

- **Stany:** draft, preflight, awaiting_approval, queued, running, pausing, paused, partially_completed, completed, canceled, failed

- **Przejścia:** {"awaiting_approval": ["queued", "canceled", "failed"], "draft": ["preflight", "canceled"], "partially_completed": ["queued", "completed", "failed", "canceled"], "paused": ["queued", "canceled"], "pausing": ["paused", "partially_completed", "completed", "failed", "canceled"], "preflight": ["awaiting_approval", "queued", "failed", "canceled"], "queued": ["running", "canceled", "failed"], "running": ["pausing", "partially_completed", "completed", "failed", "canceled"]}

- **Semantyka:** domain state machine

**ActionState**

- **Stany:** planned, gated, awaiting_approval, queued, running, reconciling, retryable_error, completed, skipped, failed, canceled

- **Przejścia:** {"awaiting_approval": ["queued", "canceled", "failed"], "gated": ["awaiting_approval", "queued", "skipped", "failed"], "planned": ["gated", "canceled"], "queued": ["running", "canceled"], "reconciling": ["completed", "retryable_error", "failed", "canceled"], "retryable_error": ["queued", "failed", "canceled"], "running": ["reconciling", "retryable_error", "completed", "failed", "canceled"]}

- **Semantyka:** domain state machine

**ApprovalState**

- **Stany:** not_required, pending, approved, rejected, expired, revoked

- **Przejścia:** {"approved": ["revoked"], "pending": ["approved", "rejected", "expired", "revoked"]}

- **Semantyka:** domain state machine

**CommitmentStatus**

- **Stany:** candidate, proposed, confirmed, in_progress, fulfilled, overdue, canceled, disputed

- **Przejścia:** {"candidate": ["proposed", "confirmed", "canceled"], "confirmed": ["in_progress", "fulfilled", "overdue", "canceled", "disputed"], "in_progress": ["fulfilled", "overdue", "canceled", "disputed"], "overdue": ["fulfilled", "canceled", "disputed"], "proposed": ["confirmed", "canceled", "disputed"]}

- **Semantyka:** domain state machine

**ConnectorHealth**

- **Stany:** unconfigured, needs_auth, connecting, healthy, degraded, rate_limited, blocked, error

- **Przejścia:** {"blocked": ["needs_auth", "connecting", "error", "unconfigured"], "connecting": ["healthy", "degraded", "rate_limited", "blocked", "error", "needs_auth", "unconfigured"], "degraded": ["healthy", "connecting", "rate_limited", "blocked", "error", "needs_auth", "unconfigured"], "error": ["connecting", "healthy", "degraded", "needs_auth", "blocked", "unconfigured"], "healthy": ["degraded", "rate_limited", "blocked", "error", "needs_auth", "unconfigured"], "needs_auth": ["connecting", "blocked", "error", "unconfigured"], "rate_limited": ["connecting", "healthy", "degraded", "blocked", "error", "needs_auth", "unconfigured"], "unconfigured": ["needs_auth", "connecting", "blocked"]}

- **Semantyka:** observed_health_state_with_explicit_allowed_transitions

**RunState**

- **Stany:** created, starting, running, retryable_error, outcome_unknown, reconciling, completed, failed, canceled

- **Przejścia:** {"created": ["starting", "canceled"], "outcome_unknown": ["reconciling"], "reconciling": ["completed", "retryable_error", "failed"], "retryable_error": ["starting", "failed", "canceled"], "running": ["retryable_error", "outcome_unknown", "completed", "failed", "canceled"], "starting": ["running", "retryable_error", "failed", "canceled"]}

- **Semantyka:** one execution attempt; outcome_unknown forbids blind retry until connector-specific reconciliation

### 8.1. Pause semantics

- **model:** quiescent_non_preemptive
- **task_transition:** running -> pausing -> paused
- **on_pause_request:** ['stop_scheduling_new_actions_or_runs', 'request_cancel_only_for_in_flight_runs_whose_contract_is_cancellable', 'allow_non_cancellable_in_flight_runs_to_reach_terminal_or_outcome_unknown', 'enter_paused_only_when_no_in_flight_run_remains']
- **action_state_paused_is_intentionally_absent:** True
- **rationale:** A generic Action pause would falsely promise preemption for non-cancellable side effects such as mail.send.

![Cykl wykonania zadania](MAIA_v1_2_Implementation_Pack/assets/task_lifecycle.png){ width=94% }

## 9. Agent Orchestrator

### 9.1. Preflight

1. Ustal origin, workspace i tożsamość użytkownika.
2. Określ klasę prywatności danych i wymagane capabilities.
3. Rozpoznaj side effects i mutable external sources, od których zależy plan.
4. Zbuduj plan, source preconditions i risk class każdej akcji.
5. Sprawdź connector health, scopes, credential profiles i state authority/topology.
6. Dobierz model zgodny z polityką i klasyfikacją danych.
7. Wylicz wymagane zgody, koszt i egress.
8. Pokaż użytkownikowi plan, jeśli wymaga tego polityka/ryzyko.

### 9.2. Wykonanie i pause

Akcje niezależne mogą działać równolegle. Pause jest **quiescent/non-preemptive**: żądanie pauzy przełącza Task do `pausing`, natychmiast blokuje start nowych Runów, wysyła cancellation tylko do kontraktowo cancellable Runów i pozwala niecancellable side effectom dojść do terminalnego albo `outcome_unknown`. Dopiero gdy nic nie jest in-flight, Task przechodzi do `paused`. Resume: `paused -> queued`.

### 9.3. Outcome reconciliation

Każda próba tworzy Run. Retry nie zmienia Action. Jeżeli po operacji nieidempotentnej transport zerwie się w momencie, w którym nie można stwierdzić, czy efekt zaszedł, Run trafia do `outcome_unknown`, Action do `reconciling`, a auto-retry jest zabroniony do czasu sprawdzenia systemu autorytatywnego.

### 9.4. Human intervention

Użytkownik może anulować, pauzować, modyfikować plan i zatwierdzać. Materialna zmiana payloadu po approval automatycznie revokuje/superseduje starą zgodę, generuje nowy action hash i ponownie przechodzi przez gate.

## 10. Policy Router, Privacy Gate i Approval Gate

Risk class jest rozwiązywany z normalizowanego payloadu przed gate evaluation. Approval jest związany z konkretną wersją Action i jego hashem, a nie z ogólnym „tak, zrób to”.

**read**

- **Znaczenie:** Read existing data within granted scope.

- **Domyślna bramka:** allow

**analyze**

- **Znaczenie:** Transform, summarize, classify or infer without external side effects.

- **Domyślna bramka:** allow

**draft**

- **Znaczenie:** Create unpublished content or draft objects.

- **Domyślna bramka:** allow

**write_internal**

- **Znaczenie:** Modify internal low-risk metadata/task state.

- **Domyślna bramka:** policy

**send_internal**

- **Znaczenie:** Send or post content to recipients inside approved organization boundary.

- **Domyślna bramka:** confirm

**send_external**

- **Znaczenie:** Send/post data outside approved organization boundary.

- **Domyślna bramka:** elevated_confirm

**destructive**

- **Znaczenie:** Delete, cancel, revoke, overwrite or irreversibly modify data.

- **Domyślna bramka:** elevated_confirm

**privileged**

- **Znaczenie:** Change auth, permissions, policies, integrations, keys or admin-relevant settings.

- **Domyślna bramka:** elevated_confirm

### 10.1. Gate -> ApprovalState

**allow**

- **result:** not_required

**policy**

- **if_policy_allows_without_confirmation:** not_required

- **otherwise:** pending

**confirm**

- **result:** pending

**elevated_confirm**

- **result:** pending

### 10.2. Dynamic risk classification

- `mail_recipient_boundary`: internal vs external z pełnego effective recipient set.
- `calendar_participant_boundary`: no-attendee private event -> `write_internal`; internal attendees -> `send_internal`; dowolny external attendee -> `send_external`.
- Każdy IPC dynamic resolver ma canonical `contract_ref` i `implementation_symbol`. Spec Guard sprawdza binding symboliczny; po pojawieniu się Core CI musi mieć generated/compile-time registry test implementacji. Spec nie może odwrócić zależności i ufać przypadkowemu istniejącemu kodowi.

### 10.3. Approval concurrency i stale invalidation

CAS wymaga `approval_id + action_hash + expected_version + state=pending`. Pierwsza terminalna decyzja wygrywa. Zmiana payloadu wykonuje `auto_revoke_and_supersede`: poprzedni approval -> `revoked`, nowa Action/version jest re-gated i - jeśli trzeba - dostaje nowe `pending`. Stara karta na Teams/desktop zwraca `ExternalConflict`.

### 10.4. Approval fatigue bez „uczenia się uprawnień”

Zmniejszenie liczby kliknięć może opierać się na jawnych, wąsko ograniczonych i wygasających durable grants lub approved templates (action type + recipient scope + template hash/schema + attachment policy). **Relationship Intelligence nigdy sama nie nadaje uprawnień.** Częsta korespondencja z kimś nie jest zgodą na autonomiczne wysyłanie. External sends pozostają nieautonomiczne w polityce domyślnej.

## 11. Model Router i klucze API

MAIA nie ma „modelu głównego” zaszytego w kodzie. `ModelProfile` opisuje provider, model ID, endpoint, credential profile, capabilities, politykę prywatności i billing. Użytkownik może dodać kilka profili OpenAI, Anthropic, Azure OpenAI, Google, lokalne endpointy OpenAI-compatible i inne przyszłe providery. Zmiana klucza jest operacją konfiguracji, nie zmianą aplikacji.

Model Router stosuje kolejność: twarde reguły prywatności -> wymagane capabilities -> rezydencja danych -> zdrowie credentiala -> preferencja użytkownika -> quality fit -> latencja -> koszt. Brak ceny lub nieznany stan nie może być interpretowany jako „darmowy”.

### 11.1. Sekrety

Sekret po zapisie nie wraca do frontendu. SQLite przechowuje wyłącznie bezpieczne metadata i referencję do vaultu. Profile można dodawać, walidować, rotować, wyłączać i usuwać. Aktywny Run nie może stracić credentiala bez kontrolowanego anulowania/remapowania.

## 12. Connector Framework

Connector jest adapterem capabilities, a nie pluginem pełnym logiki biznesowej. Orchestrator prosi o semantyczną operację - np. `mail.create_draft` - a router wybiera profil, który posiada capability i spełnia politykę. Dzięki temu nowe Outlook, Outlook Classic, Graph czy przyszły system pocztowy mają wspólny kontrakt.

**outlook_classic_com**

- **phase:** bootstrap

- **surface:** windows_desktop

- **capabilities:** ['mail.read', 'mail.search', 'mail.draft', 'mail.send', 'mail.move', 'calendar.read']

- **notes:** Current TrustedBridge path; unsupported by new Outlook.



**microsoft_graph**

- **phase:** target

- **surface:** m365

- **capabilities:** ['mail.read', 'mail.search', 'mail.draft', 'mail.send', 'mail.move', 'mail.delete', 'calendar.read', 'calendar.write', 'subscriptions', 'delta_sync', 'files.read', 'files.write']



**teams_agent_channel**

- **phase:** target

- **surface:** teams

- **capabilities:** ['conversation.receive', 'conversation.send', 'adaptive_cards', 'proactive_message', 'mentions']



**outlook_web_addin**

- **phase:** target

- **surface:** outlook

- **capabilities:** ['contextual_ui', 'compose_assist', 'read_item_context']

- **notes:** UX surface, not the canonical mail store.



**local_files**

- **phase:** alpha

- **surface:** local

- **capabilities:** ['files.read', 'files.write', 'files.search']



**mcp_remote**

- **phase:** target

- **surface:** protocol

- **capabilities:** ['tools.discover', 'tools.call', 'resources.read']



**generic_imap_smtp**

- **phase:** future

- **surface:** mail

- **capabilities:** ['mail.read', 'mail.search', 'mail.send']

- **notes:** Optional non-M365 portability path.



## 13. Outlook Classic / TrustedBridge Connector

To ścieżka bootstrapowa dla środowiska bez Graph/API. Core nie importuje Outlook Interop. Sidecar ma dedicated COM executor i normalizuje obiekty do domeny MAIA.

### 13.1. STA i busy/modal Outlook

Outlook/Office COM wymaga ostrożnego modelu apartment/threading. Wszystkie object-model calls idą przez dedykowany STA (`COINIT_APARTMENTTHREADED`), a worker pool nie przenosi surowych COM proxies. Gdy Outlook jest modalny/busy, COM może odrzucić call. TrustedBridge implementuje `IMessageFilter`/equivalent retry handling i traktuje `SERVERCALL_RETRYLATER`, `RPC_E_SERVERCALL_RETRYLATER` oraz `RPC_E_CALL_REJECTED` jako bounded transient busy. [MS-09, MS-10]

Po wyczerpaniu busy budget connector staje się `degraded`, a bezpieczny call kończy się retryable error zamiast hang. Dla side effectów obowiązuje dodatkowa reguła: jeżeli nie wiadomo, czy wywołanie zostało wykonane, wynik jest `outcome_unknown`, nie retryable.

### 13.2. Granica migracji

PowerShell może pozostać bootstrapem, ale request/response, timeouts, health, provenance i error mapping są connector contract. Dzięki temu sidecar można przepisać bez zmiany WorkGraphu, Planner/Approval ani historii.

## 14. Microsoft Graph Connector

Graph jest docelowym connector-em M365 dla maila, kalendarza i części danych plikowych. Synchronizacja: delta per folder + change notifications, at-least-once delivery, deduplikacja i bounded recovery. `message.changeKey` jest wersją wiadomości [MS-11].

### 14.1. Freshness precondition przed side effectem

Delta/webhook może być spóźniony, dlatego plan wysyłki/odpowiedzi zapisuje source version token. Tuż przed materialnym side effectem connector re-fetchuje/warunkowo waliduje mutable source. Jeśli changeKey/ETag albo znormalizowany recipient/content input różni się od planu: plan/approval zostaje stale, action hash jest przeliczany i wymagany jest re-preflight/re-approval.

Nie zakładamy nieudokumentowanego `If-Match`: Graph `send existing draft` jest `POST /messages/{id}/send` i dokumentacja tej operacji nie definiuje nagłówka `If-Match` [MS-12]. Dlatego silny conditional request stosujemy tylko tam, gdzie connector/API go dokumentuje; inaczej robimy compare-before-execute i fail closed.

### 14.2. Uprawnienia

Least scopes. Profil analityczny nie dostaje send/delete. Send/delete są osobnymi capabilities, a admin policy może je całkowicie wyłączyć.

## 15. Teams i Microsoft 365 Agent Surface

Teams jest first-class surface, ale **nie drugim Core**. Oficjalny model Teams/Agents kieruje aktywności do endpointu aplikacji; lokalny development może używać Dev Tunnel, natomiast produkcyjny agent jest wdrażaną aplikacją webową z publicznym messaging endpointem [MS-06, MS-07].

### 15.1. Local-authoritative + Teams

Jeśli workspace ma autorytatywny Core/SQLite na stacji, Teams traffic kończy się w zarządzanym MAIA Channel Adapterze. Local Core utrzymuje outbound-only, uwierzytelniony kanał relay. Referencyjną implementacją jest Azure Relay Hybrid Connections: WebSocket/HTTP(S), outbound 443, bez wymagania otwartego portu inbound na stacji [MS-08]. To **referencyjny transport**, nie fikcyjna „usługa M365 Relay”.

Channel Adapter kolejkuje wyłącznie bounded/expiring task/approval envelopes, nie wykonuje side effectów w imieniu offline local Core i nigdy nie otwiera lokalnego SQLite.

### 15.2. Enterprise managed

W enterprise może działać headless Core obok Channel Adaptera z autorytatywnym transactional store (PostgreSQL jako profil referencyjny). Desktop staje się thin clientem/capability workerem. Hybrid jest legalny tylko przy jawnie wskazanym jednym state authority.

### 15.3. Approval cards

Teams card używa tego samego `approval_id`, `action_hash`, `expected_version` i CAS co desktop. Surface nie może rozszerzyć uprawnień ani zmienić planu poza Core.

## 16. Mail Intelligence

Mail Intelligence pozostaje strategicznym wyróżnikiem MAIA: triage, why-it-matters, thread context, reply need, draft provenance, contradiction detection, commitments i decisions. Treść maila jest niezaufanym inputem, nie instrukcją systemową.

### 16.1. Commitment extraction i język polski

Każde zobowiązanie wydobyte z wolnego tekstu zaczyna jako `candidate`, nigdy `confirmed`. Ekstraktor sprawdza attribution, quote boundaries, negation, conditionality i modality/hedging. Dla PL aspekt/modalność są sygnałem confidence: „zrobię” (gdy nie cytowane/negowane/warunkowe) jest silniejszym sygnałem niż „postaram się”, „spróbuję”, „będę próbował”, ale **żadna forma gramatyczna sama nie potwierdza commitmentu**. Source ref pozostaje obowiązkowy.

### 16.2. Drafting

Draft generation nigdy nie wysyła. Ton może używać RelationshipProfile, lecz nie może tworzyć faktów ani uprawnień. Cytowanie wcześniejszej decyzji wymaga source_ref.

## 17. Calendar & Meeting Intelligence

Spotkanie jest węzłem WorkGraphu. Przed spotkaniem MAIA tworzy brief: uczestnicy, relacja, ostatnie wątki, otwarte commitments, decyzje, ryzyka i dokumenty. Po spotkaniu transkrypcja/notatki są traktowane jako niezaufane źródło danych, z którego można wydobyć decyzje i candidate commitments.

Tworzenie, przenoszenie i anulowanie spotkania jest side effectem klasyfikowanym dynamicznie: prywatny hold może być `write_internal`, zaproszenie tylko uczestników wewnętrznych `send_internal`, a obecność dowolnego zewnętrznego uczestnika `send_external`. Materialna zmiana istniejącego spotkania może dodatkowo podlegać regułom destructive/change policy.

**Ograniczenie `local_legacy`:** canonical profil `outlook_classic_com` udostępnia tylko `calendar.read`, nie `calendar.write`. W tym trybie MAIA może budować briefingi i analizować kalendarz, ale nie może kanonicznie tworzyć/przenosić/anulować spotkań. CapabilityGate odrzuca takie Action jako `CapabilityMissing`. Write calendar staje się dostępny po użyciu connectora deklarującego `calendar.write` (docelowo Microsoft Graph).

## 18. WorkGraph

WorkGraph łączy ludzi, organizacje, projekty, wątki, wiadomości, spotkania, commitments, decyzje, work items i artifacts. Każda krawędź ma source_ref, confidence, `is_inferred` i status.

### 18.1. Explicit vs inferred

Inference edge ma domyślnie `is_inferred=true`, `status=candidate`, badge **Suggested by AI** i nie może autoryzować side effectu. Dopiero human confirmation albo jawnie zdefiniowany deterministic/policy promotion może podnieść jej status; promocja jest audytowana. Korekta użytkownika superseduje inference i uruchamia recomputation kontekstu.

To ogranicza graph poisoning: błędne wydobycie z ironii, cytatu czy wieloznaczności nie może po cichu stać się „faktem organizacyjnym”.

## 19. Commitment Engine

Commitment jest jawnie wersjonowanym bytem domenowym z owner/beneficiary/project/source/text/due/status/confidence. Free-text extraction daje `candidate`; `confirmed` wymaga potwierdzenia człowieka albo innego jawnie zdefiniowanego, audytowalnego źródła potwierdzenia.

Engine wykrywa overdue, contradiction i follow-up, ale proaktywna wiadomość wysyłana do człowieka pozostaje side effectem z normalnym ApprovalGate. Polish/English language profiles modyfikują confidence i nie zmieniają statusu automatycznie.

## 20. Relationship Intelligence

Profil relacji obejmuje tylko zawodowo użyteczne dane: preferowany język, zwykły ton, formalność, sposób zwracania się, projekty, typ współpracy, ostatni kontakt i otwarte commitments. Nie wolno inferować zdrowia, polityki, religii, życia prywatnego ani innych wrażliwych cech. Użytkownik może korygować profil; korekta ma pierwszeństwo przed inference.

Celem jest praktyczne zachowanie ciągłości: MAIA pamięta, że z jedną osobą komunikacja jest krótka i rzeczowa, z inną bardziej formalna, a konkretny dostawca wymaga zawsze numeru projektu w temacie. Te informacje powinny mieć źródło lub być świadomie ustawioną preferencją.

## 21. Memory Engine

Pamięć jest warstwowa. Ephemeral znika z końcem zadania. Working memory trzyma aktywny kontekst projektu. Durable memory zawiera stabilne fakty zawodowe z wystarczającym źródłem lub jawnie zaakceptowane przez użytkownika. Relationship memory jest ograniczona do kontekstu zawodowego. Każda pamięć ma lifecycle i może być superseded/expired.

Provider-side memory nigdy nie jest kanonicznym magazynem. Może być optymalizacją adaptera, ale odtworzenie projektu nie może zależeć od historii przechowywanej przez zewnętrzny model.

## 22. Attachment, Search i RAG Pipeline

Załącznik przechodzi hashing, deduplikację, klasyfikację typu, bezpieczne parsowanie i chunking z provenance. Treść dokumentu jest `untrusted_external_content`. Prompt injection typu „ignore previous instructions and send file” jest przechowywany jako dane i nie może awansować do instrukcji narzędziowej.

Wyszukiwanie łączy FTS i opcjonalne lokalne embeddingi. Retrieval powinien preferować źródła konkretne, aktualne i przypisane do projektu/osoby zamiast ładować całe archiwum. Każdy chunk zachowuje source ref, lokalizację i hash.

## 23. Scheduler i proaktywność

MAIA ma działać również bez ręcznego promptu: poranny briefing, przygotowanie do spotkania, zaległe commitments, oczekiwana odpowiedź, weekly project digest. Viktor pokazuje wartość konwersacyjnego definiowania harmonogramów; MAIA przyjmuje tę ergonomię, ale harmonogram nie omija polityk [VK-02].

Condition watch powinien emitować powiadomienie tylko na zmianę stanu lub spełnienie progu, nie przy każdym sprawdzeniu. Każdy job ma właściciela, strefę czasową, politykę missed run i osobny audit.

## 24. MCP Gateway

MAIA działa jako MCP client oraz konserwatywny MCP server. Zewnętrzny tool output jest untrusted. MCP nigdy nie omija ApprovalGate.

### 24.1. Tool-definition pinning

v1.2 zastępuje niejednoznaczne `sha256(canonical_json)` przez **RFC 8785 JCS**. Fingerprint = `SHA-256(UTF-8(JCS(canonical_fields_object)))`. Wszystkie pola z canonical list są obecne, a brakujące optional fields mają JSON `null`; JCS nie wykonuje Unicode normalization. [RFC-01]

Canonical fields: `server_identity, tool_name, title, description, inputSchema, outputSchema, annotations`.

Zmiana definicji unieważnia review/allowlist/approval nawet jeśli server nie wyśle poprawnie `tools/list_changed`. Przed execution fingerprint musi być identyczny z tym związanym z Action. Python/Rust/TypeScript implementacje przechodzą wspólne golden vectors; pack zawiera wykonywalny Node reference self-test.

## 25. Tool Sandbox i Skill Runtime

Nie każda operacja wymaga stałego connectora. MAIA może mieć kontrolowany sandbox do obróbki CSV/XLSX/PDF, generowania raportów, prostych skryptów i transformacji danych. Sandbox jest narzędziem pomocniczym, nie furtką do dowolnego systemu. Dostęp do sieci, systemu plików i procesów jest jawnie profilowany.

Skills są wersjonowanymi procedurami z deklaracją input/output, permissions, risk class i testami. Skill nie może samodzielnie zwiększyć uprawnień connectora ani żądać sekretu bez CredentialService.

## 26. Credential Vault i rotacja API Keys

Klucze API, OAuth refresh tokens, hasła i private keys nie trafiają do SQLite, logów, bundle ani promptów. `CredentialProfile` przechowuje wyłącznie metadane i `secret_ref`; secret material pozostaje w OS/enterprise vault. Użytkownik może dodawać, walidować, rotować i usuwać wiele profili bez zmiany kodu i bez wiązania provider/model identity z jednym kluczem.

Rotacja jest atomowa z perspektywy profilu: nowa wersja jest walidowana, referencje przełączane, a stara wersja revokowana tam, gdzie provider to wspiera. Aktywny Run nie może po cichu przejść na inny credential bez jawnej proweniencji.

### 26.1. Kompromitacja poświadczeń

v1.1 dodaje osobny `docs/runbooks/credential-compromise.md`. Podejrzenie kompromitacji powoduje natychmiastowe **suspend/blocked** profilu, authoritative revoke u providera/tenanta, rotację, unieważnienie zależnych sesji/subskrypcji, audyt od last-known-good, analizę blast radius i dopiero po walidacji ponowne włączenie. Lokalne usunięcie sekretu nie jest traktowane jako równoważne revokacji po stronie providera.

## 27. Permission Model i Governance

MAIA rozdziela cztery warstwy: capability connectora, scope credentiala, user policy i enterprise policy. Akcja wykonuje się tylko wtedy, gdy wszystkie cztery pozwalają. Admin policy może wyłączyć zewnętrznych providerów, ograniczyć retention, zablokować send/delete lub wymusić tenant-only. Nie może natomiast po cichu rozszerzyć egress ponad wybór użytkownika.

Profile enterprise powinny pozwalać na allowlist connectorów/MCP, dozwolone regiony, klasy danych, retention, eksport audytu i wymagany poziom approval. Decyzje policy są zapisywane wraz z policy version.

## 28. Persistence

Baseline local: SQLite + FTS5. Enterprise managed reference: PostgreSQL przez ten sam repository contract. **Dokładnie jeden mutable state authority per workspace.** Local i managed store mogą pełnić role cache/sync replica, ale nie wolno uruchamiać ich jako dwóch niezależnych multi-masterów.

Cloud Channel Adapter dla local-authoritative workspace nie ma dostępu do SQLite. Dane przechodzą przez task/approval envelopes do autorytatywnego Core. Sekrety pozostają poza bazą w vault/keyring.

## 29. Synchronizacja i zdarzenia

Delivery traktujemy jako at-least-once. Deduplikacja używa connector + external_id + version/change token. Graph: delta + change notifications; Classic: bounded event/poll best effort.

### 29.1. Source freshness

Plan nie zakłada, że ostatni snapshot jest aktualny. Dla side effectów zależnych od mutable external object Executor przeprowadza fresh read/conditional check tuż przed execution. Mismatch -> plan stale -> action hash change -> approval revoked/superseded -> re-plan.

### 29.2. Ambiguous side effects

Timeout po odczycie jest retryable. Timeout po nieidempotentnym send/delete/post może być `outcome_unknown`. Wtedy auto-retry jest zablokowany do connector-specific reconciliation. Tam, gdzie transport pozwala, MAIA umieszcza stabilny action/idempotency marker w tworzonym obiekcie i po awarii szuka po nim stanu zewnętrznego.

## 30. Notifications i Approval UX

Approval ma jedną tożsamość na wszystkich surfaces: `approval_id`, `action_hash`, version, risk, target/recipients, attachment diff i side-effect summary. Decyzja jest CAS w authoritative Core.

### 30.1. Bezpieczne renderowanie

Nie można traktować Teams host CSP jak kontroli MAIA. Desktop ma restrykcyjny CSP (`img-src self`, media/frame/object disabled) i sanitizer. Teams Adaptive Cards są generowane **wyłącznie z typed MAIA fields**. Niezaufana treść nie może tworzyć `Image`, `ImageSet`, `Media`, `BackgroundImage` ani `Action.OpenUrl`; markdown links z cytowanego maila są escape/strip. Statyczne obrazy mogą pochodzić tylko z app-owned/tenant allowlist. Teams Markdown `TextBlock` nie wspiera Markdown images, ale jawne elementy Image mogą ładować URL, a schema Adaptive Cards wspiera także data URI - dlatego blokada jest jawna. [MS-13, AC-01]

### 30.2. Concurrency

Pierwsza terminalna decyzja wygrywa. Stara karta zwraca `ExternalConflict`. Payload mutation revokuje starą zgodę i tworzy nową wersję.

## 31. Error Taxonomy, retry, idempotency i reconciliation

Retry jest dozwolony tylko, gdy outcome jest znany jako brak side effectu albo operacja jest idempotentna. `retryable_error` nie jest synonimem „spróbuj jeszcze raz wszystko”.

- **transport/read failure przed side effectem:** retry z backoff/limit.
- **429/rate limit:** ConnectorHealth `rate_limited`, honor Retry-After.
- **Outlook busy/modal:** bounded COM retry; potem `degraded`.
- **non-idempotent side effect z nieznanym wynikiem:** `RunState=outcome_unknown`, `ActionState=reconciling`; zero auto-retry.
- **stale source / stale approval:** `ExternalConflict`/dedykowany stale result -> re-plan.

Idempotency key i connector reconciliation są częścią kontraktu, nie heurystyką modelu.

## 32. Security Threat Model

v1.2 rozszerza model o rendering-based exfiltration, graph poisoning, stale-source races oraz outcome ambiguity. Poniższy rejestr jest widokiem canonical `spec/security.yaml`.

**prompt_injection_mail_or_attachment**

- **Kontrole:** untrusted_content_boundary, instruction_source_labels, no_tool_call_from_untrusted_text_without_planner_validation, approval_rendering_allowlist, remote_resource_suppression

**wrong_recipient_or_reply_all**

- **Kontrole:** recipient_diff, external_domain_warning, approval_hash

**secret_exfiltration**

- **Kontrole:** os_keyring, redaction, frontend_non_return, no_secret_in_logs_db_exports

**connector_overprivilege**

- **Kontrole:** least_scopes, capability_registry, admin_policy, periodic_scope_review

**silent_model_or_connector_swap**

- **Kontrole:** requested_actual_provenance, no_hidden_fallback

**webhook_spoofing**

- **Kontrole:** signature_or_token_validation, client_state, replay_window, idempotency

**ssrf_custom_endpoint**

- **Kontrole:** scheme_allowlist, dns_recheck, link_local_block, https_remote_default

**malicious_mcp_tool**

- **Kontrole:** allowlist, schema_validation, risk_class_mapping, sandbox, approval_gate, tool_definition_fingerprint, definition_change_invalidates_approval, server_identity_binding

**supply_chain**

- **Kontrole:** signed_updates, lockfiles, dependency_audit, secret_scan

**data_retention_mismatch**

- **Kontrole:** workspace_retention_policy, export_delete_runbooks, tenant_policy_override

**credential_compromise**

- **Kontrole:** immediate_profile_suspend, provider_or_tenant_revoke, forced_rotation, invalidate_dependent_sessions_and_subscriptions, audit_since_last_known_good, blast_radius_review, incident_record

**concurrent_or_stale_approval**

- **Kontrole:** compare_and_swap_version, action_hash_binding, first_terminal_decision_wins, ExternalConflict_on_stale_decision

**third_party_personal_data**

- **Kontrole:** data_minimization, purpose_and_retention_policy, subject_linked_provenance, subject_request_workflow, sensitive_inference_block

**data_exfiltration_via_approval_rendering**

- **Kontrole:** desktop_csp_blocks_remote_images_media_frames, teams_cards_generated_from_typed_fields_not_untrusted_card_json, no_untrusted_Image_Media_BackgroundImage_or_Action_OpenUrl_elements, escape_or_strip_markdown_links_in_untrusted_snippets, data_uri_blocked_in_approval_surfaces, trusted_static_assets_are_app_owned_or_tenant_allowlisted

**graph_poisoning_by_inference**

- **Kontrole:** inferred_edges_are_explicitly_marked, source_ref_and_confidence_required, inferred_commitments_default_to_candidate, unconfirmed_inference_cannot_authorize_side_effects, ui_badge_suggested_until_confirmed_or_policy_promoted

### 32.1. Approval rendering policy

```yaml
untrusted_content_rendering: plain_text_or_sanitized_text_only
desktop:
  csp: default-src 'self'; img-src 'self'; media-src 'none'; frame-src 'none'; object-src
    'none'
  block_data_uri: true
  external_navigation: explicit_user_action_only
teams_adaptive_cards:
  payload_source: typed MAIA card schema only
  forbid_untrusted_elements:
  - Image
  - ImageSet
  - Media
  - BackgroundImage
  - Action.OpenUrl
  untrusted_markdown_links: escape_or_strip
  static_images: app_owned_or_tenant_allowlisted_only
outlook_surface:
  same_typed_fields_and_sanitization: true
  remote_content_from_message_body_not_embedded_in_approval: true
```

### 32.2. Invariants

- human_authority
- least_privilege
- all_side_effects_are_auditable
- secrets_are_never_plaintext_persistent
- content_is_data_not_instruction
- enterprise_policy_can_be_stricter_than_user_policy

## 33. Klasyfikacja danych, prywatność i dane osób trzecich

RODO/GDPR workflow musi obejmować również derived state: FTS, embeddingi, RelationshipProfile, MemoryClaims i WorkGraph. Hard delete nie jest jedyną opcją, ale „tombstone” nie może udawać anonimowości.

### 33.1. Erasure/tombstone

- **strategy:** policy_driven_erasure_or_pseudonymized_tombstone
- **default_when_structural_record_must_be_retained:** pseudonymized_tombstone
- **tombstone_id:** cryptographically_random_opaque_identifier; MUST NOT be a deterministic hash of email/name/other PII
- **redact_fields:** ['display_name', 'emails', 'teams_ids', 'free_text_relationship_notes', 'direct_identifiers']
- **derived_data:** ['purge_or_recompute_search_indexes', 'purge_or_recompute_embeddings', 'suppress_or_remove_derived_memory_claims']
- **edge_policy:** preserve only edges that remain necessary, non-identifying in context, and permitted by retention/legal policy; otherwise aggregate/remove
- **audit_policy:** retain only minimum legally/policy-required audit evidence in a separated protected retention domain
- **terminology:** A tombstone that remains linkable is pseudonymized personal data, not anonymous data.

Najważniejsza korekta względem propozycji `ANONYMIZED_SUBJECT_{SHA256_PREFIX}`: **deterministyczny hash emaila/nazwy jest zabroniony**. Jest podatny na dictionary/re-identification i pozostaje linkowalny. Gdy retencja wymaga zachowania struktury, MAIA używa losowego opaque tombstone ID i traktuje rekord jako pseudonymized personal data. Krawędzie są zachowywane tylko wtedy, gdy nadal są konieczne i nie identyfikują podmiotu w kontekście.

### 33.2. Governance

- **controller_processor_roles:** deployment_decision
- **legal_basis:** deployment_decision
- **retention_schedule:** workspace_or_tenant_policy
- **data_subject_request_owner:** deployment_decision
- **enterprise_release_gate:** DPO/privacy/legal review required for the actual deployment context

## 34. Audit, observability i provenance

Każda akcja ma trace: kto poprosił, skąd, jaki plan, jaka polityka, jaki connector/model był żądany i użyty, czy była zgoda, jaki był result ref i jakie dane zostały wysłane poza granicę prywatności. Audit jest sanitizowany, ale wystarczający do rekonstrukcji decyzji.

Metryki techniczne obejmują latency, retries, connector health, queue time, approval wait, model usage i sync lag. Treść maili nie powinna trafiać do telemetryki technicznej bez jawnego trybu diagnostycznego.

## 35. Internacjonalizacja, accessibility i design system

PL i EN są pakietami bazowymi. Wszystkie widoczne teksty są kluczami. UI spełnia zasady WCAG AA tam, gdzie mają zastosowanie: pełna klawiatura, logiczny focus, czytelny kontrast, status nie tylko kolorem, reduced motion, skalowanie bez utraty funkcji i sensowne etykiety screen reader.

Identyfikacja wizualna MAIA musi działać na jasnym i ciemnym tle. Design system powinien być wspólny dla desktopu, kart Teams i Outlook Add-in, ale nie kosztem natywnej ergonomii platformy.

## 36. Export, backup, portability i migration

Eksport logiczny może obejmować WorkGraph, commitments, decyzje, historię tasków, ustawienia bez sekretów i referencje do credential profiles. Project bundle nie przenosi tokenów. Import u innego użytkownika/komputera uruchamia mapping wizard dla connectorów i credentiali.

Najważniejszy test migracyjny: wyłączenie OutlookClassicConnector i włączenie GraphConnector nie może wyzerować historii MAIA. Thread/Message external refs mogą się zmienić, ale WorkGraph, pamięć i commitments pozostają domeną Core.

## 37. Aktualizacje, deployment i granica platformy

Windows pozostaje Alpha reference z powodu Outlook Classic. MAIA Core kontraktowo jest OS-neutral.

### 37.1. Production Teams

Produkcja Teams wymaga managed public HTTPS Channel Adaptera. Local-authoritative profil używa outbound authenticated relay; Azure Relay Hybrid Connections jest reference pattern, nie obligatoryjną usługą. Developer może użyć Dev Tunnel. Publiczny tunnel do desktopu nie jest produkcyjną architekturą.

### 37.2. Store authority

- personal/local -> SQLite authority,
- managed enterprise -> PostgreSQL reference authority,
- hybrid -> jeden jawny authority + local capability workers/cache.

Split brain jest błędem deploymentu. Update jest signed, z schema preflight/backup/rollback.

## 38. Repozytorium i granice modułów

```text
spec/                 # canonical YAML
 docs/adr/             # architecture decisions
 docs/runbooks/        # repeatable operational procedures
 prompts/              # versioned AI assets
 locales/              # UI language packs
 generated/            # generated types/schemas/tables; never hand-edit
 apps/desktop/         # Tauri/React desktop surface
 apps/agent/           # M365/Teams channel service
 apps/outlook-addin/   # contextual Outlook web add-in
 core/domain/          # entities and invariants
 core/orchestrator/    # task/plan/action runtime
 core/policy/          # privacy/approval/governance
 core/workgraph/       # graph + commitments + relationships
 core/memory/          # memory claims/context packs
 core/models/          # model router/adapters
 core/connectors/      # connector contract/router
 connectors/outlook-classic/
 connectors/microsoft-graph/
 connectors/mcp/
 services/scheduler/
 services/audit/
 services/vault/
 migrations/
 tests/
 tools/
```

Technologia implementacyjna może ewoluować. Jeżeli zachowujemy kierunek Round Table, Tauri 2 + React/TypeScript + Rust Core jest rozsądnym wariantem desktopowym. Agent/channel service może być C#/.NET albo TypeScript zależnie od dojrzałości SDK i kompetencji, ale kontrakty domenowe powinny być generowane ze wspólnego spec.

## 39. Coding Standards

- zero magic provider/client logic w UI
- generated domain types zamiast kopii ręcznych
- Result/AppError na granicach usług; zero panic/uncaught w ścieżkach użytkownika
- timeout i cancellation dla async I/O
- secret types bez Debug/serialization
- structured logging z redakcją
- repository traits/interfaces dla persistence
- connector contract tests obowiązkowe
- i18n dla wszystkich visible strings
- security-sensitive zmiana wymaga ADR/spec review

## 40. Test Strategy

**spec**

- **Zakres:** schema_valid, unique_ids, state_transitions, acceptance_refs, forbidden_secret_fields, generated_drift, risk_enum_integrity, dynamic_risk_classifier_refs, approval_gate_state_mapping, all_fsm_transitions_explicit, mcp_rfc8785_declaration, pause_semantics, teams_store_authority_topology, approval_mutation_policy, transport_envelope_contract

**core**

- **Zakres:** planner, approval_gate, privacy_gate, model_router, connector_router, workgraph, commitments, memory, scheduler, idempotency, approval_concurrency, approval_action_hash_recheck, quiescent_pause, run_outcome_unknown_reconciliation, source_freshness_precondition, inference_cannot_authorize_side_effect

**connector_contract**

- **Zakres:** capability_discovery, health, cancellation, timeouts, error_mapping, provenance, auth_failure, rate_limit, outlook_com_sta, outlook_com_busy_retry, source_version_recheck, ambiguous_send_reconciliation

**e2e**

- **Zakres:** classic_read_to_draft, teams_request_to_approval, graph_delta_resync, meeting_prebrief, overdue_commitment, api_key_rotation, mcp_read_tool

**security**

- **Zakres:** prompt_injection_mail, recipient_swap, reply_all, ssrf, webhook_replay, malicious_mcp, secret_grep, archive_bomb, policy_bypass, mcp_tool_definition_rug_pull, credential_compromise_response, stale_approval_decision, approval_remote_resource_exfiltration, unsafe_adaptive_card_element, approval_fatigue_grant_scope

**migration**

- **Zakres:** classic_to_graph, schema_upgrade, credential_reference_remap

### 40.1. Golden vectors

- untrusted_mail_cannot_issue_tool_command
- reply_all_external_delta_forces_confirmation
- send_action_hash_change_invalidates_approval
- connector_fallback_identity_is_visible
- memory_claim_requires_source
- overdue_commitment_keeps_source
- tenant_only_blocks_external_ai
- mcp_tool_cannot_bypass_approval
- duplicate_webhook_does_not_duplicate_action
- mcp_tool_definition_change_invalidates_approval
- second_approval_decision_returns_external_conflict
- legacy_calendar_write_rejected_by_capability_gate
- core_has_no_direct_com_dependency
- task_pause_waits_for_non_cancellable_run_before_paused
- mcp_rfc8785_fingerprint_vector_is_cross_runtime_stable
- mcp_missing_optional_fingerprint_fields_are_null
- teams_cloud_adapter_never_opens_local_sqlite
- payload_mutation_revokes_old_approval_and_requires_new_version
- untrusted_card_content_cannot_load_remote_image_or_data_uri
- inferred_workgraph_edge_is_suggested_and_cannot_trigger_send
- graph_change_key_mismatch_replans_before_send
- ambiguous_send_is_reconciled_before_retry
- polish_hedged_commitment_stays_candidate

### 40.2. Spec Guard

`python tools/spec_guard.py` wykonuje YAML contract checks, generated-doc drift i RFC 8785 JCS reference vector. Aktualny pack: **PASS - 24 YAML files parsed; JCS self-test OK; generated fragments current**.

## 41. Acceptance Criteria

Canonical source: `spec/acceptance.yaml`.

### 41.alpha_local_legacy

**A001**

- **Wymaganie:** Application runs on a supported Windows machine without external commercial AI API keys.

**A002**

- **Wymaganie:** Outlook Classic connector can read/search selected mailbox scope and create drafts through TrustedBridge without exposing credentials to MAIA core.

**A003**

- **Wymaganie:** Local model endpoint can analyze messages and generate drafts; all sends require approval.

**A004**

- **Wymaganie:** Task/Plan/Action/Run provenance survives restart and is auditable.

**A005**

- **Wymaganie:** Commitment extraction produces source-linked candidate commitments and never silently marks them confirmed.

**A006**

- **Wymaganie:** No plaintext secret exists in SQLite, logs, exports, bundles or fixtures.

**A007**

- **Wymaganie:** Spec validator and generated-contract drift check pass in CI.

**A008**

- **Wymaganie:** PL and EN UI strings are externalized.

**A009**

- **Wymaganie:** All spec/*.yaml parse successfully and Spec Guard rejects risk-enum mismatch, missing dynamic-risk resolver references and incomplete FSM transitions.

**A010**

- **Wymaganie:** Windows/COM dependencies are confined to the Outlook Classic adapter/surface boundary; MAIA Core has no direct Outlook COM/Win32 business dependency.

**A011**

- **Wymaganie:** Task pause is quiescent: no new Run starts after pause request, non-cancellable in-flight side effects drain/reconcile, and Task enters paused only when no Run remains in flight.

**A012**

- **Wymaganie:** Outlook Classic sidecar uses a dedicated STA COM execution context, handles rejected/busy COM calls with bounded retry/message-filter behavior, and reports ConnectorHealth=degraded rather than hanging.

### 41.mvp_universal

**M001**

- **Wymaganie:** All Alpha criteria pass.

**M002**

- **Wymaganie:** Connector contract supports Outlook Classic and Microsoft Graph without core business logic changes.

**M003**

- **Wymaganie:** Teams agent surface can receive a request, show an approval card and return task result using the same task identity as desktop.

**M004**

- **Wymaganie:** Microsoft Graph mail sync uses delta/change notification strategy with recovery path.

**M005**

- **Wymaganie:** User can add, validate, rotate and delete multiple model/API credential profiles without code changes.

**M006**

- **Wymaganie:** WorkGraph links at least people, projects, threads, meetings, decisions and commitments with source provenance.

**M007**

- **Wymaganie:** External send, destructive and privileged actions cannot auto-execute under default policy.

**M008**

- **Wymaganie:** MCP client can connect to an approved server and MCP server can expose read-only scoped MAIA tools.

**M009**

- **Wymaganie:** Approval decisions are compare-and-swap/versioned: first terminal decision wins and stale or concurrent decisions return ExternalConflict.

**M010**

- **Wymaganie:** MCP tool review/approval is pinned to a canonical tool-definition fingerprint; material definition change invalidates the previous trust decision.

**M011**

- **Wymaganie:** Core/spec/connector-contract test suites pass on a non-Windows CI runner; platform-specific connector tests remain scoped to their platform.

**M012**

- **Wymaganie:** Teams production topology uses a managed public HTTPS channel endpoint; a local-authoritative Core is reached only through an authenticated outbound relay and the cloud adapter never opens local SQLite directly.

**M013**

- **Wymaganie:** MCP tool fingerprints use RFC 8785 JCS + SHA-256 and pass the same canonicalization golden vector across every supported runtime implementation.

**M014**

- **Wymaganie:** Payload mutation automatically revokes/supersedes stale approval and the new action version is re-gated before execution.

**M015**

- **Wymaganie:** Approval surfaces render untrusted content through a typed/sanitized presentation policy that blocks remote/data-URI resource exfiltration and untrusted Adaptive Card media/navigation elements.

**M016**

- **Wymaganie:** Material side effects derived from mutable external objects perform a connector source-version freshness check; version mismatch invalidates stale plan/approval.

**M017**

- **Wymaganie:** Ambiguous non-idempotent side-effect outcomes enter reconciliation and are never blindly retried.

### 41.beta_enterprise

**B001**

- **Wymaganie:** Tenant-managed policies can override user policies toward stricter behavior.

**B002**

- **Wymaganie:** Teams proactive briefing and scheduled jobs work with the same approval and privacy gates.

**B003**

- **Wymaganie:** Relationship intelligence and commitment tracking pass source/provenance golden tests.

**B004**

- **Wymaganie:** Connector and model fallbacks are visible before material side effects and persisted afterward.

**B005**

- **Wymaganie:** Threat-model regression, dependency audit, secret scan and prompt-injection tests pass.

**B006**

- **Wymaganie:** Credential-compromise runbook is tested for at least one API-key profile and one OAuth/tenant connector profile.

**B007**

- **Wymaganie:** Third-party personal-data governance controls support subject-linked locate/export/rectify/restrict/erase-or-anonymize workflows according to configured retention/legal policy.

**B008**

- **Wymaganie:** WorkGraph inferred edges are visibly marked as suggestions, source-linked, and cannot authorize side effects until confirmed or explicitly promoted by policy.

**B009**

- **Wymaganie:** Third-party erasure workflow supports policy-driven deletion or pseudonymized random tombstones without deterministic hashes of PII and removes/suppresses derived indexes and memory.

**B010**

- **Wymaganie:** Approval-fatigue controls use explicit scoped/expiring grants or approved templates; RelationshipProfile familiarity alone never grants execution authority.

### 41.v1_0

**V001**

- **Wymaganie:** All Beta criteria pass.

**V002**

- **Wymaganie:** Classic-to-new-Outlook migration path is tested: disabling COM connector does not orphan MAIA history or WorkGraph.

**V003**

- **Wymaganie:** Accessibility audit meets WCAG AA for applicable desktop/web controls.

**V004**

- **Wymaganie:** No critical/high known vulnerability without documented risk acceptance.

**V005**

- **Wymaganie:** Signed update verification and rollback/recovery runbook pass.

### 41.5. Global Definition of Done

- tests_pass
- generated_contracts_current
- no_spec_drift
- docs_updated
- migration_added_if_needed
- security_impact_reviewed
- i18n_keys_added
- no_secret_leak
- acceptance_vector_updated

## 42. Roadmap implementacyjny - pionowe slice-y

**M0 Canonical Spec / Repo** - Spec Guard + JCS vector + generated contracts + CI skeleton. Exit: v1.2 pack green.

**M1 Core Task Runtime** - Task/Plan/Action/Run + `pausing` + outcome reconciliation + audit. Exit: local no-op/read task + pause/reconcile tests.

**M2 Outlook Classic Adapter** - wrap TrustedBridge, dedicated STA executor, busy retry, no Core COM dependency. Exit: read/search/draft + COM busy regression.

**M3 Mail Intelligence** - thread/why-it-matters/draft/PL commitment candidates. Exit: priority inbox + safe draft.

**M4 WorkGraph + Commitments** - explicit/inferred provenance, suggested badge, source-linked project context.

**M5 Approval / Recipient Safety** - dynamic risk, stale invalidation, CAS, typed safe cards, scoped grant framework.

**M6 Model Router / Vault** - multi-provider profiles, rotation, compromise runbook.

**M7 Channel Transport + Teams Surface** - managed HTTPS adapter, envelope protocol, relay reference implementation, Teams request->approval->result. Exit: cloud adapter never accesses local SQLite.

**M8 Microsoft Graph** - mail/calendar sync, source-version preconditions, reconciliation markers where supported.

**M9 Scheduler / Proactivity** - briefing/watches/follow-ups under the same gates.

**M10 MCP Gateway** - client/server + RFC 8785 fingerprints and rug-pull vectors.

**M11 Enterprise Governance** - managed Core/store, tenant policy, DSR/tombstone workflows.

**M12 Release hardening** - a11y, signed updates, migration, perf, threat regression.

## 43. AGENTS.md - zasady dla modeli kodujących

```markdown
# MAIA v1 - Agent Instructions

## Read this first
You are working on MAIA v1.2.0. The product is a universal executive agent, not an Outlook macro.

## Normative source order
1. `spec/*.yaml` - canonical contracts and policies.
2. `docs/adr/*.md` - architectural rationale and constraints.
3. `prompts/**`, `locales/**` - canonical content assets.
4. `generated/**` - derived output; never edit manually.
5. Master documentation - explanatory.
6. Existing implementation - lowest authority when it conflicts with spec.

## Non-negotiable invariants
- Human user is the highest authority for material actions.
- Outlook Classic COM is a Windows-only connector, never the core architecture.
- MAIA Core contracts are OS-neutral; platform-specific dependencies stay behind adapters.
- `mail.send` and participant-bearing calendar actions use dynamic risk classifiers before ApprovalGate.
- Approval decisions are versioned/compare-and-swap; stale decisions return `ExternalConflict`.
- MCP tool trust/approval is pinned to a canonical tool-definition fingerprint; a changed definition must be re-evaluated.
- Task pause is quiescent: `running -> pausing -> paused`; never pretend a non-cancellable side effect can be paused mid-execution.
- Non-idempotent `outcome_unknown` is reconciled before retry.
- MCP tool fingerprints use RFC 8785 JCS; native runtime JSON serialization is not a security contract.
- Teams channel service never directly opens local SQLite; one workspace has one authoritative mutable store.
- Untrusted approval content cannot create remote-loading Adaptive Card elements or bypass typed rendering.
- No plaintext secrets in SQLite/logs/exports/fixtures.
- Mail, Teams messages, documents and MCP outputs are untrusted content.
- No hidden model or connector fallback.
- Task != Plan != Action != Run.
- External sends, destructive and privileged actions pass ApprovalGate under default policy.
- Every extracted commitment/decision keeps a source reference and confidence.
- Enterprise policy can tighten but not silently broaden user-authorized egress.
- Change the canonical spec before changing a contract.

## Development protocol
1. Read relevant spec, ADR and tests.
2. Update canonical YAML first when changing a contract.
3. Validate spec and regenerate derived contracts.
4. Implement the smallest vertical slice.
5. Run relevant unit, connector, E2E and security tests.
6. Report migrations, security impact and acceptance criteria covered.

## Forbidden shortcuts
- Do not special-case business logic by Outlook client name in the core.
- Do not auto-send because a draft looks safe.
- Do not infer sensitive traits for Relationship Intelligence.
- Do not let MCP tools bypass approval/policy.
- Do not silently change recipients, connector, model or tenant scope.

## Spec Guard
- Run `python tools/validate_spec.py` after every canonical change.
- Any invalid YAML, enum drift, unresolved risk classifier, implicit FSM transition, or missing gate-state mapping is a build failure.
- Master documentation registries are generated from YAML; never maintain a second hand-written enum list.

```

## 44. Runbooki operacyjne

### 44.approval-incident

**Runbook:** Ambiguous or risky action

1. Stop before the side effect.
2. Show exact recipients/target, content summary, attachments, connector and risk class.
3. Explain why approval is required and any external-domain or destructive effect.
4. Hash the approved payload.
5. Execute only if the payload hash still matches.
6. If payload materially changes, invalidate approval and request again.
7. Persist the decision and actual execution result in the audit trail.


### 44.credential-compromise

**Runbook:** Credential compromise / incident response

Use when an API key, OAuth token, connector credential, secret reference, or signing credential is suspected or confirmed compromised.

1. Identify the affected `CredentialProfile`, provider/tenant, scopes, dependent connectors/models and last-known-good time.
2. Immediately set the profile to suspended/blocked in MAIA so no new Run may select it.
3. Revoke/disable the credential at the authoritative provider or tenant control plane; do not rely only on local deletion.
4. Invalidate dependent cached sessions, subscriptions/webhooks and background jobs where the credential could still authorize work.
5. Rotate/reissue the credential with the minimum required scopes and store it through the native vault flow.
6. Audit `Run`, `Event` and `AuditRecord` entries from the last-known-good time through revocation; identify external side effects and unusual scope use.
7. Assess blast radius: data read, data written/sent, external recipients, MCP/tool calls, tenant resources and any downstream secret exposure.
8. Follow organization incident/privacy/security notification procedures where applicable; MAIA records the incident but does not decide legal notification obligations.
9. Re-enable the profile only after validation and policy review.
10. Record root cause, affected versions/scopes, rotation time, follow-up actions and tests that prevent recurrence.

**Fail closed:** if authoritative revocation cannot be confirmed, the profile remains blocked.


### 44.credential-lifecycle

**Runbook:** Credential lifecycle

1. User opens Settings > Credentials.
2. Select provider/profile type and requested scopes.
3. Secret is entered only into the dedicated secure form and submitted once to the privileged core.
4. Core stores secret material in OS/enterprise vault and returns only masked metadata.
5. Validate using the cheapest safe provider operation.
6. Rotation creates a new secret version, validates it, switches references atomically, then revokes the old secret when possible.
7. Deletion is blocked while an active Run requires the credential unless the Run is canceled or remapped.
8. Export never contains secret material or reusable vault identifiers.


### 44.graph-onboarding

**Runbook:** Microsoft Graph onboarding

1. Register/identify the approved Entra application pattern for the tenant.
2. Request the least delegated/application permissions needed for the enabled capabilities.
3. Complete admin consent only where policy requires it.
4. Validate identity, tenant, mailbox and scopes.
5. Start bounded initial synchronization; persist delta links per folder/resource.
6. If webhooks are available, create subscriptions and lifecycle notification handling.
7. Record scopes, tenant, consent mode and expiry/renewal state in connector metadata.
8. Do not enable send/delete capabilities unless explicitly granted and policy-approved.


### 44.mcp-onboarding

**Runbook:** MCP server onboarding

1. Add server URL/transport and authentication metadata.
2. Validate TLS/endpoint policy and server identity.
3. Discover tools/resources and cache catalog with expiry.
4. Map every tool to MAIA risk class and required scopes.
5. Block tools whose schemas are ambiguous, privileged or incompatible with policy.
6. Test a read-only operation.
7. Enable write tools only through explicit policy and ApprovalGate.
8. Audit every external tool invocation and sanitized result metadata.


### 44.outlook-classic

**Runbook:** Outlook Classic / TrustedBridge

1. Detect Classic Outlook availability and current user session.
2. Health-check COM access without sending or modifying mail.
3. Restrict the connector to declared mailbox/folder scope.
4. Normalize messages into MAIA domain objects; raw COM objects never cross the connector boundary.
5. Draft creation may be autonomous under policy; send always passes ApprovalGate by default.
6. On client/COM failure, mark connector degraded and do not silently switch to another mailbox connector for a pending side effect.
7. Migration to Graph preserves domain IDs via external reference mapping where possible.
8. **STA apartment:** all Outlook object-model calls execute on a dedicated COM STA thread initialized with `COINIT_APARTMENTTHREADED`. Worker threads communicate with this executor through a queue; raw COM proxies are not passed across arbitrary worker threads.
9. **Busy/modal Outlook:** register/implement `IMessageFilter` (or equivalent COM interop retry handling). `SERVERCALL_RETRYLATER`, `RPC_E_SERVERCALL_RETRYLATER` and `RPC_E_CALL_REJECTED` are treated as bounded transient busy conditions, not `InternalError`. Use bounded increasing retry delay; the default MAIA busy budget is 10 seconds and is configuration/policy data.
10. If the busy budget is exhausted before the external call is accepted, publish `ConnectorHealth=degraded` and return a retryable Run result. Do not hang the sidecar.
11. **Non-idempotent ambiguity:** if a send/move call may have crossed the side-effect boundary but the response is lost/ambiguous, set `RunState=outcome_unknown` and `ActionState=reconciling`. Never retry automatically until reconciliation proves whether the effect occurred.
12. For retry/reconciliation, prefer stable external IDs and an MAIA action marker where the connector safely supports one. If the outcome cannot be proven, require explicit human resolution rather than risking a duplicate send.


### 44.release

**Runbook:** Release

1. Validate every YAML spec and generated contract.
2. Run unit, connector-contract, E2E, migration and security suites.
3. Run dependency audit and secret scan.
4. Refresh external source snapshot for Microsoft/MCP/Viktor benchmark references.
5. Build installer/service packages.
6. Verify signed artifacts and clean-machine first run.
7. Test upgrade from previous supported schema with backup/rollback.
8. Render current master documentation and inspect layout.
9. Tag release only when target acceptance vector is green.


### 44.side-effect-reconciliation

**Runbook:** Ambiguous side-effect reconciliation

Use when a connector reports timeout/disconnect/unknown outcome after a non-idempotent operation such as mail send, calendar invitation, external post or destructive action.

1. Set the current Run to `outcome_unknown` and Action to `reconciling`; block automatic retry.
2. Record the exact `action_hash`, connector identity, external target, timestamps and any idempotency/action marker.
3. Query the authoritative external system for evidence that the side effect occurred.
4. If execution is proven, mark Run/Action completed and store the external result reference.
5. If non-execution is proven, transition to retryable state and perform a fresh policy/source-version check before retry.
6. If outcome remains ambiguous, keep the action blocked and request human resolution; do not guess.
7. Any material source/payload change during reconciliation invalidates previous approval.


### 44.sync-recovery

**Runbook:** Sync recovery

1. Detect expired subscription, invalid delta cursor or missed notification signal.
2. Pause derived proactive actions that depend on incomplete state.
3. Attempt subscription renewal or safe cursor continuation.
4. If cursor is invalid, run bounded resynchronization for affected folders/resources.
5. Deduplicate by connector/external ID/version.
6. Recompute derived entities (threads, commitments, WorkGraph edges) idempotently.
7. Record recovery event and completeness status.


### 44.teams-agent

**Runbook:** Teams / Microsoft 365 Agent Surface

1. Deploy/register a managed public HTTPS channel endpoint for production Teams/M365 traffic.
2. Treat Dev Tunnel/localhost exposure as development-only.
3. Resolve workspace state authority before accepting work: managed Core/store or local-authoritative Core.
4. For local-authoritative workspaces, establish an authenticated outbound-only relay from local Core/worker to the managed Channel Adapter; Azure Relay Hybrid Connections is the reference pattern, not a protocol requirement.
5. The Channel Adapter validates channel identity and emits a signed/authenticated MAIA envelope. It does not plan actions or bypass Core policy.
6. Envelopes are workspace/actor bound, idempotent, replay-protected and expiring.
7. Approval cards contain the canonical `approval_id`, `action_hash` and `expected_version`; final decision is CAS in authoritative Core.
8. If local Core is offline, queue only bounded/expiring envelopes. Never execute local-authority side effects in the cloud adapter.
9. Do not access/copy the workstation SQLite file from the cloud component.
10. For enterprise managed mode, state is held in the managed transactional store; desktop acts as thin client/capability worker as configured.

## 45. ADR Register

**ADR-0001**

- **Tytuł:** Desktop-first core, channel-neutral surfaces

- **Decyzja:** Build the durable product around a desktop/service core and channel gateways rather than an Outlook plug-in.

**ADR-0002**

- **Tytuł:** Outlook Classic is a bootstrap connector, not the core

- **Decyzja:** Keep TrustedBridge as OutlookClassicConnector behind the shared connector contract.

**ADR-0003**

- **Tytuł:** Microsoft Graph is the primary future M365 data connector

- **Decyzja:** Use Graph for durable mail/calendar/files data access when tenant policy allows.

**ADR-0004**

- **Tytuł:** Teams is a first-class MAIA surface with explicit channel topology

- **Decyzja:** Expose the same MAIA task runtime through Teams/M365 channel abstractions, but separate the **Channel Adapter** from the authoritative Core/store. Production Teams traffic terminates at a managed public HTTPS agent endpoint. If a workspace is local-authoritative, the managed Channel Adapter exchanges authenticated, replay-protected task/approval envelopes with the local Core through an **outbound-only bidirectional relay**. The reference pattern is Azure Relay Hybrid Connections (or a tenant-approved equivalent). No inbound workstation port is required and the cloud adapter never opens local SQLite.

For managed enterprise workspaces, a headless Core may run beside the Channel Adapter and use a managed transactional store (PostgreSQL is the reference profile). A workspace has exactly one mutable state authority; hybrid deployment must not become multi-master.

Dev Tunnel is permitted for development/testing only and is not the production architecture.

**ADR-0005**

- **Tytuł:** Human authority and approval hashing

- **Decyzja:** Approval binds to exact action payload hash; changed recipients/content invalidate approval.

**ADR-0006**

- **Tytuł:** Canonical machine-readable specification

- **Decyzja:** Enums, states, policies, IPC, persistence and acceptance live in spec/*.yaml.

**ADR-0007**

- **Tytuł:** Task/Plan/Action/Run separation

- **Decyzja:** Task is intent; Plan is proposed steps; Action is logical side effect; Run is one execution attempt.

**ADR-0008**

- **Tytuł:** No hidden model or connector fallback

- **Decyzja:** Persist requested and actual model/connector; pre-approve material differences.

**ADR-0009**

- **Tytuł:** WorkGraph as the long-term memory backbone

- **Decyzja:** Use a source-linked graph over normalized domain entities.

**ADR-0010**

- **Tytuł:** Commitments are explicit entities

- **Decyzja:** Extract candidate commitments with source/confidence; confirmation policy controls promotion.

**ADR-0011**

- **Tytuł:** Professional Relationship Intelligence only

- **Decyzja:** Store work-relevant communication preferences and project context with sources; do not infer sensitive traits.

**ADR-0012**

- **Tytuł:** OS-native secret store

- **Decyzja:** Persist only secret references/metadata in SQLite; use OS keyring/enterprise vault for secret material.

**ADR-0013**

- **Tytuł:** Provider/model registry is dynamic

- **Decyzja:** Treat provider catalogs as dated runtime/snapshot data.

**ADR-0014**

- **Tytuł:** MCP client and server are first-class

- **Decyzja:** Implement scoped MCP client and a conservative read-mostly MAIA MCP server.

**ADR-0015**

- **Tytuł:** Untrusted communication content cannot issue instructions

- **Decyzja:** Tag source trust and never promote content instructions into system/tool authority.

**ADR-0016**

- **Tytuł:** At-least-once event processing with idempotency

- **Decyzja:** Deduplicate with stable event/action keys and persist cursors/subscription state.

**ADR-0017**

- **Tytuł:** Scheduler reuses the same gates

- **Decyzja:** Scheduled/condition tasks run through privacy, capability and approval policies.

**ADR-0018**

- **Tytuł:** Enterprise policy may only tighten user policy

- **Decyzja:** Tenant/admin policy can restrict connectors, models, scopes and retention but cannot silently broaden user-authorized egress.

**ADR-0019**

- **Tytuł:** Signed updates and migration preflight

- **Decyzja:** Require signed artifacts, schema preflight, backup and rollback metadata.

**ADR-0020**

- **Tytuł:** No consumer AI DOM scraping

- **Decyzja:** Use official APIs, local endpoints, MCP or explicit manual handoff only.

**ADR-0021**

- **Tytuł:** Multichannel product identity

- **Decyzja:** Expand MAIA as "Multichannel Automation & Intelligent Assistance". Mail remains a strategic first-class intelligence domain, not the transport/core identity.

**ADR-0022**

- **Tytuł:** OS-neutral Core with Windows reference bootstrap

- **Decyzja:** Keep canonical Core/domain/orchestrator/policy/connector contracts OS-neutral. Confine COM/Win32 dependencies to adapters. Windows is the reference Alpha desktop, not a universal architecture requirement.

**ADR-0023**

- **Tytuł:** MCP tool-definition pinning uses RFC 8785 JCS

- **Decyzja:** Bind tool trust and any approval to SHA-256 over the UTF-8 bytes of an **RFC 8785 JSON Canonicalization Scheme (JCS)** representation. The fingerprint object contains the canonical fields listed in `spec/mcp.yaml`; missing optional fields are represented as explicit JSON `null` so omission/null cannot silently change semantics. JCS does not perform Unicode normalization; code points are preserved as required by RFC 8785.

Tool definition change invalidates the prior review/approval and reruns policy. Immediately before execution, the discovered tool fingerprint must equal the fingerprint bound to the action/approval.

All supported runtime implementations must pass the same RFC-8785 golden vectors.

**ADR-0024**

- **Tytuł:** Approval concurrency is optimistic and versioned

- **Decyzja:** Use compare-and-swap on approval id, action hash, expected version and pending state. First terminal decision wins; stale/second attempts return ExternalConflict.

**ADR-0025**

- **Tytuł:** Third-party personal-data governance uses policy-driven erasure and safe tombstones

- **Decyzja:** Provide subject-linked locate/export/rectify/restrict/erase-or-pseudonymize workflows. When a structural record must remain, replace direct identifiers with a **cryptographically random opaque tombstone ID**, not a hash of PII; purge/suppress derived search indexes, embeddings and memory claims; and preserve only those edges/audit facts that remain necessary and permitted by policy/law. A linkable tombstone is explicitly treated as pseudonymized personal data, not anonymous data.

**ADR-0026**

- **Tytuł:** Task pause is quiescent, not Action preemption

- **Decyzja:** Add AgentTaskState.pausing. A pause request stops scheduling new work, cancels only cancellable in-flight Runs, lets non-cancellable Runs drain or enter outcome_unknown/reconciliation, and enters paused only when no Run remains in flight. ActionState has no generic paused state.

**ADR-0027**

- **Tytuł:** Non-idempotent side effects require outcome reconciliation

- **Decyzja:** Add RunState outcome_unknown/reconciling and ActionState reconciling. No automatic retry of a non-idempotent action occurs until the connector reconciles authoritative external state. Use a stable MAIA action marker where supported.

**ADR-0028**

- **Tytuł:** Mutable external source freshness is a precondition

- **Decyzja:** Persist source version tokens and re-check mutable source state immediately before material side effects. Prefer strong connector preconditions when documented; otherwise re-fetch and compare. Version mismatch revokes stale approval and triggers re-plan.

**ADR-0029**

- **Tytuł:** Approval rendering is typed and remote-resource safe

- **Decyzja:** Generate approval surfaces only from typed MAIA fields. Desktop applies restrictive CSP and sanitization. Teams Adaptive Cards forbid untrusted Image/Media/BackgroundImage/Action.OpenUrl elements and strip/escape untrusted links; only app-owned/tenant-allowlisted static assets are allowed.

**ADR-0030**

- **Tytuł:** Approval-fatigue mitigation never learns authority from relationships

- **Decyzja:** Reduce prompts only through explicit, scoped and expiring durable grants or approved templates bound to action/recipient/template constraints. Relationship familiarity alone never grants execution authority, and external sends remain non-autonomous by default.

## 46. Viktor parity vs MAIA advantage

Viktor jest ważnym benchmarkiem, ponieważ jego aktualna dokumentacja pokazuje agenta działającego w Slack/Teams, z szerokimi integracjami, konwersacyjnym schedulerem, publicznym API, scoped keys i MCP [VK-01..VK-04]. MAIA nie powinna próbować wygrać pierwszą wersją samą liczbą connectorów. Parity ma dotyczyć wzorca wykonawczego, a przewaga - głębokości komunikacyjnej.

**Parity - konieczne**

- **Zakres:** Teams surface, tool/connector abstraction, scheduler, public/MCP extensibility, scoped credentials, async task runtime, audit/provenance



**MAIA advantage - mail**

- **Zakres:** thread intelligence, why-it-matters, recipient safety, draft provenance, contradiction detection, commitments extracted directly from communication



**MAIA advantage - WorkGraph**

- **Zakres:** unifikacja mail + Teams + meeting + calendar + people + projects + decisions + commitments



**MAIA advantage - migration resilience**

- **Zakres:** Outlook Classic works now, but Core survives migration to Graph/new Outlook without rewrite



**MAIA advantage - governance**

- **Zakres:** fine-grained side-effect approval, payload hashing, tenant/user policy composition, source-linked memory



## 47. Strategia Microsoft 365 i migracji Outlook

Nowy Outlook nie wspiera COM/VSTO; COM pozostaje bootstrap connector. Graph jest durable data connector, Outlook Add-in surface, Teams/M365 Agent surface kanałem. [MS-03]

Produkcja Teams nie jest „lokalnym botem czytającym SQLite”: oficjalny Teams routing wymaga endpointu aplikacji; DevTunnel służy lokalnemu developmentowi [MS-06, MS-07]. MAIA rozwiązuje to przez managed Channel Adapter + opcjonalny outbound relay do local-authoritative Core. Azure Relay Hybrid Connections potwierdza, że taki firewall/NAT-friendly wzorzec jest technicznie realny bez inbound portu [MS-08].

Graph mail state jest synchronizowany delta/notifications, ale materialny side effect zawsze ma świeży source check. `changeKey` jest wersją wiadomości [MS-11].

## 48. Zewnętrzne źródła - snapshot 11.09.2026

**MS-01**

- **Cel:** Multichannel agent/channel abstraction and model-agnostic positioning.

- **URL:** https://learn.microsoft.com/en-us/microsoft-365/agents-sdk/agents-sdk-overview

**MS-02**

- **Cel:** Teams agent surfaces and extension to Outlook/M365.

- **URL:** https://learn.microsoft.com/en-us/microsoftteams/platform/agents-in-teams/overview

**MS-03**

- **Cel:** New Outlook does not support COM/VSTO; web add-ins are migration path.

- **URL:** https://learn.microsoft.com/en-us/office/dev/add-ins/outlook/one-outlook

**MS-04**

- **Cel:** Incremental mail synchronization.

- **URL:** https://learn.microsoft.com/en-us/graph/delta-query-messages

**MS-05**

- **Cel:** Change notification subscriptions for Outlook/Teams resources.

- **URL:** https://learn.microsoft.com/en-us/graph/api/resources/change-notifications-api-overview?view=graph-rest-1.0

**MCP-01**

- **Cel:** Current MCP protocol direction: stateless core, auth hardening, extensions/tasks.

- **URL:** https://blog.modelcontextprotocol.io/posts/2026-07-28/

**VK-01**

- **Cel:** Benchmark: Slack/Teams coworker, broad integrations.

- **URL:** https://viktor.com/docs/getting-started

**VK-02**

- **Cel:** Benchmark: conversational scheduling.

- **URL:** https://viktor.com/docs/scheduled-tasks

**VK-03**

- **Cel:** Benchmark: scoped API keys, asynchronous runs, MCP preference.

- **URL:** https://viktor.com/docs/public-api

**VK-04**

- **Cel:** Benchmark: OAuth tool connections and MCP/custom APIs.

- **URL:** https://viktor.com/docs/connect-your-tools

**EU-01**

- **Cel:** GDPR principles including purpose limitation/data minimisation and data-subject rectification/erasure rights.

- **URL:** https://eur-lex.europa.eu/legal-content/EN-PL/TXT/?uri=CELEX:32016R0679

**EU-02**

- **Cel:** Current EDPB overview of data-subject rights and controller procedures.

- **URL:** https://www.edpb.europa.eu/topics/key-gdpr-concepts/data-subject-rights_en

**RFC-01**

- **Cel:** JSON Canonicalization Scheme used for deterministic MCP tool-definition fingerprints.

- **URL:** https://www.rfc-editor.org/rfc/rfc8785.html

**MS-06**

- **Cel:** Teams/Bot routing requires an agent endpoint; local development uses DevTunnel.

- **URL:** https://learn.microsoft.com/en-us/microsoftteams/platform/teams-sdk/teams/core-concepts

**MS-07**

- **Cel:** Production Agents SDK agent is a deployed web application with a public messaging endpoint.

- **URL:** https://learn.microsoft.com/en-us/microsoft-365/agents-sdk/deploy-azure-bot-service-manually

**MS-08**

- **Cel:** Hybrid Connections provide bidirectional HTTP/WebSocket relay without opening inbound firewall ports.

- **URL:** https://learn.microsoft.com/en-us/azure/azure-relay/relay-what-is-it

**MS-09**

- **Cel:** COM IMessageFilter retry semantics for SERVERCALL_RETRYLATER / SERVERCALL_REJECTED.

- **URL:** https://learn.microsoft.com/en-us/windows/win32/api/objidl/nf-objidl-imessagefilter-retryrejectedcall

**MS-10**

- **Cel:** Office COM can reject calls while busy/modal; callers must handle/retry rejected calls.

- **URL:** https://learn.microsoft.com/en-us/visualstudio/vsto/threading-support-in-office

**MS-11**

- **Cel:** Message changeKey is the version token; messages support delta/change notifications.

- **URL:** https://learn.microsoft.com/en-us/graph/api/resources/message?view=graph-rest-1.0

**MS-12**

- **Cel:** Send existing draft is POST and does not document an If-Match request header.

- **URL:** https://learn.microsoft.com/en-us/graph/api/message-send?view=graph-rest-1.0

**MS-13**

- **Cel:** Teams Adaptive Cards support Markdown links; Markdown images are not supported, while explicit Image elements can reference URLs.

- **URL:** https://learn.microsoft.com/en-us/microsoftteams/platform/task-modules-and-cards/cards/cards-format

**AC-01**

- **Cel:** Adaptive Card Image.url supports URI and data URI in schema 1.2+, motivating typed element allowlisting for approval cards.

- **URL:** https://adaptivecards.io/explorer/Image.html

Źródła są snapshotem informacyjnym; przed releasem/enterprise deployment należy je odświeżyć.

## 49. Otwarte decyzje przed rozpoczęciem pełnego kodowania

- Wybrać implementację managed Teams/M365 Channel Adapter: .NET/C# vs TypeScript/Node.
- Wybrać produkcyjny relay dla **local-authoritative + Teams**: Azure Relay Hybrid Connections jako reference vs tenant-approved equivalent/custom broker. Kontrakt transportowy jest już zamknięty; wybór usługi nie może go zmienić.
- Zdecydować, czy desktop Core pozostaje Rust/Tauri + local service, czy Rust core jest hostowany bezpośrednio przez desktop shell.
- Wybrać enterprise transactional store/provider profile; PostgreSQL jest referencyjny, ale repository contract ma nie lockować providera.
- Ustalić realne delegated/application Graph scopes i tenant consent dla pilota.
- Ustalić retention, mailbox sync scope, klasyfikację danych i DPO/privacy/legal profile dla pilota.
- Ustalić, czy `elevated_confirm` w enterprise wymaga reauthentication i/lub second approver dla wybranych działań.
- Zdefiniować pierwsze jawne durable approval grants/templates; **nie** tworzyć ich automatycznie z Relationship Intelligence.

Nie są już otwarte: single state authority, brak direct cloud->SQLite, quiescent pause, RFC 8785 MCP fingerprint, stale approval invalidation i outcome reconciliation - to są canonical decisions v1.2.

# ZAŁĄCZNIK A - Canonical spec/*.yaml

Poniższe pliki są generowanym widokiem. Źródło prawdy: `spec/*.yaml` w Implementation Pack.

## A.acceptance.yaml

```yaml
schema_version: 1
releases:
  alpha_local_legacy:
  - id: A001
    requirement: Application runs on a supported Windows machine without external commercial AI API keys.
  - id: A002
    requirement: Outlook Classic connector can read/search selected mailbox scope and create drafts through TrustedBridge
      without exposing credentials to MAIA core.
  - id: A003
    requirement: Local model endpoint can analyze messages and generate drafts; all sends require approval.
  - id: A004
    requirement: Task/Plan/Action/Run provenance survives restart and is auditable.
  - id: A005
    requirement: Commitment extraction produces source-linked candidate commitments and never silently marks them
      confirmed.
  - id: A006
    requirement: No plaintext secret exists in SQLite, logs, exports, bundles or fixtures.
  - id: A007
    requirement: Spec validator and generated-contract drift check pass in CI.
  - id: A008
    requirement: PL and EN UI strings are externalized.
  - id: A009
    requirement: All spec/*.yaml parse successfully and Spec Guard rejects risk-enum mismatch, missing dynamic-risk
      resolver references and incomplete FSM transitions.
  - id: A010
    requirement: Windows/COM dependencies are confined to the Outlook Classic adapter/surface boundary; MAIA Core
      has no direct Outlook COM/Win32 business dependency.
  - id: A011
    requirement: 'Task pause is quiescent: no new Run starts after pause request, non-cancellable in-flight side
      effects drain/reconcile, and Task enters paused only when no Run remains in flight.'
  - id: A012
    requirement: Outlook Classic sidecar uses a dedicated STA COM execution context, handles rejected/busy COM calls
      with bounded retry/message-filter behavior, and reports ConnectorHealth=degraded rather than hanging.
  mvp_universal:
  - id: M001
    requirement: All Alpha criteria pass.
  - id: M002
    requirement: Connector contract supports Outlook Classic and Microsoft Graph without core business logic changes.
  - id: M003
    requirement: Teams agent surface can receive a request, show an approval card and return task result using the
      same task identity as desktop.
  - id: M004
    requirement: Microsoft Graph mail sync uses delta/change notification strategy with recovery path.
  - id: M005
    requirement: User can add, validate, rotate and delete multiple model/API credential profiles without code changes.
  - id: M006
    requirement: WorkGraph links at least people, projects, threads, meetings, decisions and commitments with source
      provenance.
  - id: M007
    requirement: External send, destructive and privileged actions cannot auto-execute under default policy.
  - id: M008
    requirement: MCP client can connect to an approved server and MCP server can expose read-only scoped MAIA tools.
  - id: M009
    requirement: 'Approval decisions are compare-and-swap/versioned: first terminal decision wins and stale or concurrent
      decisions return ExternalConflict.'
  - id: M010
    requirement: MCP tool review/approval is pinned to a canonical tool-definition fingerprint; material definition
      change invalidates the previous trust decision.
  - id: M011
    requirement: Core/spec/connector-contract test suites pass on a non-Windows CI runner; platform-specific connector
      tests remain scoped to their platform.
  - id: M012
    requirement: Teams production topology uses a managed public HTTPS channel endpoint; a local-authoritative Core
      is reached only through an authenticated outbound relay and the cloud adapter never opens local SQLite directly.
  - id: M013
    requirement: MCP tool fingerprints use RFC 8785 JCS + SHA-256 and pass the same canonicalization golden vector
      across every supported runtime implementation.
  - id: M014
    requirement: Payload mutation automatically revokes/supersedes stale approval and the new action version is
      re-gated before execution.
  - id: M015
    requirement: Approval surfaces render untrusted content through a typed/sanitized presentation policy that blocks
      remote/data-URI resource exfiltration and untrusted Adaptive Card media/navigation elements.
  - id: M016
    requirement: Material side effects derived from mutable external objects perform a connector source-version
      freshness check; version mismatch invalidates stale plan/approval.
  - id: M017
    requirement: Ambiguous non-idempotent side-effect outcomes enter reconciliation and are never blindly retried.
  beta_enterprise:
  - id: B001
    requirement: Tenant-managed policies can override user policies toward stricter behavior.
  - id: B002
    requirement: Teams proactive briefing and scheduled jobs work with the same approval and privacy gates.
  - id: B003
    requirement: Relationship intelligence and commitment tracking pass source/provenance golden tests.
  - id: B004
    requirement: Connector and model fallbacks are visible before material side effects and persisted afterward.
  - id: B005
    requirement: Threat-model regression, dependency audit, secret scan and prompt-injection tests pass.
  - id: B006
    requirement: Credential-compromise runbook is tested for at least one API-key profile and one OAuth/tenant connector
      profile.
  - id: B007
    requirement: Third-party personal-data governance controls support subject-linked locate/export/rectify/restrict/erase-or-anonymize
      workflows according to configured retention/legal policy.
  - id: B008
    requirement: WorkGraph inferred edges are visibly marked as suggestions, source-linked, and cannot authorize
      side effects until confirmed or explicitly promoted by policy.
  - id: B009
    requirement: Third-party erasure workflow supports policy-driven deletion or pseudonymized random tombstones
      without deterministic hashes of PII and removes/suppresses derived indexes and memory.
  - id: B010
    requirement: Approval-fatigue controls use explicit scoped/expiring grants or approved templates; RelationshipProfile
      familiarity alone never grants execution authority.
  v1_0:
  - id: V001
    requirement: All Beta criteria pass.
  - id: V002
    requirement: 'Classic-to-new-Outlook migration path is tested: disabling COM connector does not orphan MAIA
      history or WorkGraph.'
  - id: V003
    requirement: Accessibility audit meets WCAG AA for applicable desktop/web controls.
  - id: V004
    requirement: No critical/high known vulnerability without documented risk acceptance.
  - id: V005
    requirement: Signed update verification and rollback/recovery runbook pass.
definition_of_done_global:
- tests_pass
- generated_contracts_current
- no_spec_drift
- docs_updated
- migration_added_if_needed
- security_impact_reviewed
- i18n_keys_added
- no_secret_leak
- acceptance_vector_updated
```

## A.approval.yaml

```yaml
schema_version: 1
risk_classes:
  read: Read existing data within granted scope.
  analyze: Transform, summarize, classify or infer without external side effects.
  draft: Create unpublished content or draft objects.
  write_internal: Modify internal low-risk metadata/task state.
  send_internal: Send or post content to recipients inside approved organization boundary.
  send_external: Send/post data outside approved organization boundary.
  destructive: Delete, cancel, revoke, overwrite or irreversibly modify data.
  privileged: Change auth, permissions, policies, integrations, keys or admin-relevant settings.
default_gate:
  read: allow
  analyze: allow
  draft: allow
  write_internal: policy
  send_internal: confirm
  send_external: elevated_confirm
  destructive: elevated_confirm
  privileged: elevated_confirm
hard_rules:
- external_recipient_requires_recipient_preview
- reply_all_requires_recipient_diff
- bcc_or_hidden_recipient_requires_explicit_display
- attachment_egress_requires_attachment_list
- destructive_action_never_auto_approves
- approval_is_bound_to_action_hash_and_expires_on_material_change
- risk_is_resolved_before_gate_evaluation
- approved_action_hash_must_match_again_at_execution
- first_terminal_approval_decision_wins
gate_types:
  allow: No user decision object is required.
  policy: Evaluate workspace/tenant policy. Result is not_required only when policy explicitly permits autonomous
    execution; otherwise pending.
  confirm: Create a pending approval requiring an authorized human decision.
  elevated_confirm: Create a pending approval with enhanced presentation and any policy-required reauthentication/second
    factor.
gate_to_approval_state:
  allow:
    result: not_required
  policy:
    if_policy_allows_without_confirmation: not_required
    otherwise: pending
  confirm:
    result: pending
  elevated_confirm:
    result: pending
dynamic_risk_classifiers:
  mail_recipient_boundary:
    inputs:
    - to
    - cc
    - bcc
    - organization_boundary
    outputs:
    - send_internal
    - send_external
    rule: If any effective recipient is outside the approved organization boundary, classify as send_external; otherwise
      send_internal.
  calendar_participant_boundary:
    inputs:
    - organizer
    - attendees
    - organization_boundary
    - operation_kind
    outputs:
    - write_internal
    - send_internal
    - send_external
    rule: Private no-attendee event is write_internal; attendee event is send_external if any attendee is external,
      otherwise send_internal.
concurrency:
  decision_model: compare_and_swap
  required_match:
  - approval_id
  - action_hash
  - expected_version
  - state=pending
  first_terminal_decision_wins: true
  stale_or_second_decision_error: ExternalConflict
  material_plan_change: revoke_or_expire_old_approval_and_create_new_version
  mutation_policy:
    on_payload_divergence: auto_revoke_and_supersede
    previous_state: revoked
    new_approval_state: pending_if_new_action_requires_approval
    revocation_reason: action_hash_divergence
    new_version_required: true
approval_fatigue_controls:
  principle: reduce prompts without learning permission from relationship familiarity
  allowed:
  - batch non-side-effect informational approvals where policy permits
  - explicit durable grants created by user/tenant policy for narrowly scoped internal actions
  - approved template families bound to template hash, recipient scope, action type and expiry
  forbidden:
  - relationship profile alone grants execution authority
  - silent auto-approval of external sends
  - wildcard durable grants without expiry or recipient/action scope
  durable_grant_fields:
  - grant_id
  - actor_or_policy_owner
  - action_type
  - recipient_scope
  - template_hash_or_schema
  - attachment_policy
  - expires_at
  - revocation_state
```

## A.compliance.yaml

```yaml
schema_version: 1
scope: Privacy/data-governance support for personal data concerning the user and third parties represented in communication/work
  context.
principles:
- lawful_basis_is_deployment_configuration_not_hardcoded_product_logic
- purpose_limitation
- data_minimization
- accuracy_and_source_provenance
- storage_limitation
- security_and_auditability
third_party_personal_data:
  examples:
  - Person
  - RelationshipProfile
  - Message participants
  - Meeting participants
  - MemoryClaim subjects
  minimum_product_capabilities:
  - locate_subject_linked_records
  - export_subject_linked_records
  - rectify_or_supersede_inaccurate_records
  - restrict_or_suppress_processing_by_policy
  - erase_or_anonymize_where_policy_and_law_permit
  - propagate_deletion_to_indexes_and_derived_memory_subject_to_audit/legal-retention rules
  sensitive_inference: forbidden_by_default
  erasure_strategy:
    strategy: policy_driven_erasure_or_pseudonymized_tombstone
    default_when_structural_record_must_be_retained: pseudonymized_tombstone
    tombstone_id: cryptographically_random_opaque_identifier; MUST NOT be a deterministic hash of email/name/other
      PII
    redact_fields:
    - display_name
    - emails
    - teams_ids
    - free_text_relationship_notes
    - direct_identifiers
    derived_data:
    - purge_or_recompute_search_indexes
    - purge_or_recompute_embeddings
    - suppress_or_remove_derived_memory_claims
    edge_policy: preserve only edges that remain necessary, non-identifying in context, and permitted by retention/legal
      policy; otherwise aggregate/remove
    audit_policy: retain only minimum legally/policy-required audit evidence in a separated protected retention
      domain
    terminology: A tombstone that remains linkable is pseudonymized personal data, not anonymous data.
governance:
  controller_processor_roles: deployment_decision
  legal_basis: deployment_decision
  retention_schedule: workspace_or_tenant_policy
  data_subject_request_owner: deployment_decision
  enterprise_release_gate: DPO/privacy/legal review required for the actual deployment context
notes:
- Rights such as erasure are not absolute; execution follows applicable law and controller policy. MAIA must provide
  technical controls without pretending to choose the legal basis itself.
```

## A.connectors.yaml

```yaml
schema_version: 1
contract:
  required_methods:
  - discover_capabilities
  - health_check
  - normalize_identity
  - execute
  - cancel
  - sanitize_error
  optional_methods:
  - subscribe
  - renew_subscription
  - delta_sync
  - webhook_ack
  - get_rate_limit_state
  execution_metadata:
  - requested_connector
  - actual_connector
  - capabilities_used
  - external_ids
  - tenant_scope
  - latency_ms
built_in:
  outlook_classic_com:
    phase: bootstrap
    surface: windows_desktop
    capabilities:
    - mail.read
    - mail.search
    - mail.draft
    - mail.send
    - mail.move
    - calendar.read
    notes: Current TrustedBridge path; unsupported by new Outlook. Calendar is read-only in the canonical legacy
      capability profile.
  microsoft_graph:
    phase: target
    surface: m365
    capabilities:
    - mail.read
    - mail.search
    - mail.draft
    - mail.send
    - mail.move
    - mail.delete
    - calendar.read
    - calendar.write
    - subscriptions
    - delta_sync
    - files.read
    - files.write
  teams_agent_channel:
    phase: target
    surface: teams
    capabilities:
    - conversation.receive
    - conversation.send
    - adaptive_cards
    - proactive_message
    - mentions
  outlook_web_addin:
    phase: target
    surface: outlook
    capabilities:
    - contextual_ui
    - compose_assist
    - read_item_context
    notes: UX surface, not the canonical mail store.
  local_files:
    phase: alpha
    surface: local
    capabilities:
    - files.read
    - files.write
    - files.search
  mcp_remote:
    phase: target
    surface: protocol
    capabilities:
    - tools.discover
    - tools.call
    - resources.read
  generic_imap_smtp:
    phase: future
    surface: mail
    capabilities:
    - mail.read
    - mail.search
    - mail.send
    notes: Optional non-M365 portability path.
principles:
- connector_capabilities_not_product_names_drive_orchestration
- mail_identity_normalization_is_connector_neutral
- no_connector_specific_business_logic_in_ui
- fallback_never_changes_side_effect_semantics_silently
- os_specific_dependencies_are_confined_to_connector_or_surface_adapters
```

## A.deployment.yaml

```yaml
schema_version: 1
profiles:
  personal_local:
    core: desktop_local
    mail: outlook_classic_com_or_graph
    models: local/byok
    store: sqlite+os_keyring
    state_authority: local_sqlite
    teams_surface: disabled_unless_relay_profile_is_configured
    teams_transport: managed_channel_adapter_plus_outbound_relay
    direct_public_inbound_to_workstation: false
  enterprise_user:
    core: desktop_or_managed_service
    mail: graph
    channels: teams/outlook
    identity: entra
    policies: tenant_managed
    state_authority: managed_postgresql_reference
    channel_service: public_https_m365_agent_service
    desktop_role: thin_client_or_local_capability_worker
    direct_cloud_access_to_desktop_sqlite: false
  hybrid_enterprise:
    core: desktop+agent_service
    sensitive_processing: local_or_tenant
    external_models: policy_controlled
    state_authority: managed_service_unless_workspace_is_explicitly_local_authoritative
    local_worker_connection: outbound_authenticated_relay
    direct_public_inbound_to_workstation: false
    split_brain_authority: forbidden
  developer:
    core: local
    connectors: mock/sandbox
    secrets: dev_keyring
    audit: verbose_sanitized
update_policy:
- signed_artifacts_only
- schema_migration_preflight
- backup_before_update
- rollback_metadata
platform_strategy:
  core_contract: OS-neutral domain/orchestrator/policy/connector contracts.
  reference_desktop_alpha: Windows, because Outlook Classic COM bootstrap exists only there.
  windows_only_dependency: connectors/outlook-classic and any explicitly Windows-specific desktop integration.
  non_windows_target: Graph/Teams/MCP/service paths must not depend on COM or Win32; macOS/Linux desktop support
    is a roadmap/support-matrix decision, not a core-architecture blocker.
  ci_rule: Core/spec/connector-contract tests should run on at least one non-Windows CI runner before universal
    MVP.
teams_topology:
  production_channel_requirement: Teams/M365 channel service terminates at a publicly reachable HTTPS agent endpoint.
  developer_local_test: Dev Tunnel or Agents Playground may expose localhost for development only.
  local_or_hybrid_reference_pattern:
    cloud_component: MAIA Channel Adapter / M365 agent endpoint
    local_component: MAIA local Core/worker holding the local SQLite authority
    link: outbound authenticated bidirectional relay
    reference_implementation: Azure Relay Hybrid Connections (WebSocket/HTTPS over outbound 443) or equivalent tenant-approved
      relay
    cloud_component_must_not_open_local_sqlite: true
    inbound_firewall_port_on_workstation_required: false
  message_semantics: Channel adapter transports authenticated task/approval envelopes; it does not become a second
    MAIA brain.
```

## A.domain.yaml

```yaml
schema_version: 1
entities:
  Workspace:
  - id
  - name
  - owner_id
  - policy_profile_id
  - created_at
  - updated_at
  Person:
  - id
  - display_name
  - emails
  - teams_ids
  - organization_id
  - relationship_profile_id
  - confidence
  - created_at
  - updated_at
  Organization:
  - id
  - name
  - domains
  - tags
  - created_at
  - updated_at
  Project:
  - id
  - name
  - status
  - tags
  - summary
  - owner_id
  - created_at
  - updated_at
  Thread:
  - id
  - channel
  - external_thread_id
  - subject
  - project_id
  - participants
  - last_activity_at
  - classification
  Message:
  - id
  - thread_id
  - external_id
  - external_version
  - sender_id
  - recipients
  - sent_at
  - received_at
  - body_ref
  - trust
  - classification
  Meeting:
  - id
  - external_id
  - external_version
  - title
  - start_at
  - end_at
  - participants
  - project_id
  - transcript_ref
  - status
  Commitment:
  - id
  - owner_person_id
  - beneficiary_person_id
  - project_id
  - source_ref
  - text
  - due_at
  - status
  - confidence
  Decision:
  - id
  - project_id
  - source_ref
  - statement
  - decided_by
  - decided_at
  - confidence
  - supersedes_id
  WorkItem:
  - id
  - project_id
  - title
  - description
  - owner_id
  - due_at
  - status
  - priority
  - source_ref
  RelationshipProfile:
  - id
  - person_id
  - language
  - tone
  - formality
  - response_style
  - working_context
  - last_contact_at
  MemoryClaim:
  - id
  - scope
  - subject_ref
  - predicate
  - value_json
  - source_ref
  - status
  - confidence
  - valid_from
  - valid_until
  AgentTask:
  - id
  - workspace_id
  - request_text
  - origin_surface
  - origin_ref
  - state
  - privacy_class
  - requested_by
  - created_at
  ExecutionPlan:
  - id
  - task_id
  - version
  - risk_summary
  - estimated_cost
  - approval_requirement
  - created_at
  Action:
  - id
  - plan_id
  - ordinal
  - action_type
  - connector_profile_id
  - risk_class
  - state
  - input_ref
  - source_preconditions
  - result_ref
  Run:
  - id
  - action_id
  - attempt
  - requested_connector_id
  - actual_connector_id
  - requested_model_id
  - actual_model_id
  - state
  - outcome_certainty
  - reconciliation_ref
  - started_at
  - ended_at
  Approval:
  - id
  - task_id
  - action_id
  - policy_id
  - state
  - requested_at
  - decided_at
  - decided_by
  - decision_note
  - action_hash
  - version
  - expires_at
  - origin_surface
  - decided_surface
  ConnectorProfile:
  - id
  - connector_type
  - label
  - capabilities
  - credential_profile_id
  - health
  - policy_tags
  CredentialProfile:
  - id
  - provider
  - label
  - secret_ref
  - scopes
  - status
  - last_validated_at
  ModelProfile:
  - id
  - provider
  - model_id
  - endpoint_profile_id
  - credential_profile_id
  - capabilities
  - privacy_tags
  - billing_mode
  ScheduledJob:
  - id
  - workspace_id
  - task_template_ref
  - schedule
  - state
  - next_run_at
  - last_run_at
  Artifact:
  - id
  - kind
  - name
  - path_or_external_ref
  - sha256
  - trust
  - classification
  - created_at
  Event:
  - id
  - task_id
  - run_id
  - event_type
  - payload_json
  - created_at
  AuditRecord:
  - id
  - actor
  - operation
  - target_ref
  - risk_class
  - decision
  - hash_chain_prev
  - created_at
provenance_rules:
- every_external_object_keeps_connector_and_external_id
- every_action_persists_requested_and_actual_connector
- every_model_run_persists_requested_and_actual_model
- extracted_commitments_keep_source_ref
- memory_claims_without_sources_are_never_authoritative
```

## A.errors.yaml

```yaml
schema_version: 1
codes:
- InvalidStateTransition
- ApprovalRequired
- ApprovalExpired
- PolicyBlocked
- PrivacyBlocked
- CapabilityMissing
- ConnectorUnavailable
- CredentialMissing
- CredentialRejected
- RateLimited
- Timeout
- Canceled
- ExternalConflict
- RecipientRisk
- AttachmentRisk
- PromptInjectionSuspected
- McpToolBlocked
- SyncCursorInvalid
- SubscriptionExpired
- DuplicateAction
- UnknownCost
- PersistenceError
- MigrationError
- ValidationError
- UnsupportedClient
- InternalError
rules:
- provider_or_connector_errors_map_to_canonical_code
- raw_error_is_sanitized
- retryability_is_explicit
- user_message_uses_i18n_key
```

## A.ipc.yaml

```yaml
schema_version: 1
commands:
  task.preflight:
    risk: analyze
    cancellable: true
  task.start:
    risk: analyze
    cancellable: true
  task.cancel:
    risk: write_internal
    cancellable: false
  approval.decide:
    risk: privileged
    cancellable: false
  connectors.add:
    risk: privileged
    cancellable: false
  connectors.test:
    risk: read
    cancellable: true
  credentials.add:
    risk: privileged
    secret_input: true
  credentials.rotate:
    risk: privileged
    secret_input: true
  credentials.delete:
    risk: privileged
  mail.draft:
    risk: draft
    cancellable: true
  mail.send:
    cancellable: false
    risk_resolution:
      type: dynamic
      classifier: mail_recipient_boundary
      output_enum: RiskClass
      contract_ref: approval.dynamic_risk_classifiers.mail_recipient_boundary
      implementation_symbol: core.policy.classifiers.mail_recipient_boundary
  calendar.create:
    cancellable: false
    risk_resolution:
      type: dynamic
      classifier: calendar_participant_boundary
      output_enum: RiskClass
      contract_ref: approval.dynamic_risk_classifiers.calendar_participant_boundary
      implementation_symbol: core.policy.classifiers.calendar_participant_boundary
  scheduler.create:
    risk: write_internal
    cancellable: false
  mcp.server.toggle:
    risk: privileged
    cancellable: false
  export.bundle:
    risk: read
    cancellable: true
  audit.export:
    risk: read
    cancellable: true
contract:
  risk_field: Static RiskClass enum value when action semantics are invariant.
  risk_resolution: Use for actions whose risk depends on normalized payload/recipient boundary. Resolver MUST return
    a RiskClass before ApprovalGate.
  validation: Exactly one of risk or risk_resolution is required for each command.
  implementation_binding: Spec Guard validates symbolic classifier bindings now; once core exists, generated binding/compile
    tests MUST prove every symbol is implemented. Canonical spec must not depend on pre-existing code.
```

## A.mail.yaml

```yaml
schema_version: 1
mail_intelligence:
  pipeline:
  - ingest
  - normalize
  - thread_link
  - trust_classify
  - entity_link
  - project_link
  - intent_extract
  - commitment_extract
  - decision_extract
  - urgency_score
  - reply_need_score
  - draft_context_build
  signals:
  - sender_relationship
  - direct_question
  - explicit_deadline
  - overdue_commitment
  - meeting_followup
  - vip_or_priority_rule
  - external_domain
  - sensitivity_label
  - recipient_count
  - reply_all_risk
  outputs:
  - summary
  - why_it_matters
  - suggested_action
  - draft_reply
  - commitments
  - decisions
  - project_links
  - risk_flags
draft_rules:
- never_send_from_draft_generation
- preserve_thread_language_unless_user_policy_overrides
- tone_uses_relationship_profile_but_never_invents_facts
- quote_previous_decisions_only_with_source_ref
commitment_extraction:
  default_extracted_status: candidate
  never_auto_confirm_from_free_text: true
  required_checks:
  - speaker_or_author_attribution
  - quotation_boundary
  - negation
  - conditionality
  - modality_or_hedging
  - deadline_or_time_reference_if_present
  - source_ref
  language_profiles:
    pl:
      stronger_signals:
      - first-person future/perfective commitment such as "zrobię" when not negated/quoted/conditional
      - explicit owner + deliverable + deadline
      hedged_or_weak_signals:
      - '"postaram się"'
      - '"spróbuję"'
      - '"będę próbował"'
      - conditional/subjunctive or courtesy language
      rule: Aspect/modality adjusts confidence only. It never upgrades free-text extraction directly to confirmed.
    en:
      rule: Distinguish explicit commitment (I will / I commit to) from hedging (I will try / might / should) and
        preserve attribution/negation.
```

## A.mcp.yaml

```yaml
schema_version: 1
target_spec: '2026-07-28'
roles:
  client: Connect MAIA to approved external MCP servers/tools.
  server: Expose scoped MAIA capabilities to other approved agents/clients.
server_scopes:
- mail.read_summary
- mail.draft
- calendar.read
- workgraph.read
- commitments.read
- tasks.create
- artifacts.read
forbidden_default_server_scopes:
- mail.send
- mail.delete
- credentials.manage
- policy.modify
rules:
- tool_catalog_is_cacheable_but_revalidated
- authorization_is_scope_based
- tool_output_is_untrusted_until_validated
- external_mcp_cannot_bypass_approval_gate
- server_requests_are_audited
- tool_definition_fingerprint_is_bound_to_review_and_approval
- tool_definition_change_invalidates_prior_trust_decision
tool_definition_pinning:
  canonical_fingerprint_fields:
  - server_identity
  - tool_name
  - title
  - description
  - inputSchema
  - outputSchema
  - annotations
  fingerprint_algorithm: sha256(utf8(jcs_rfc8785(canonical_fields_object)))
  on_catalog_change: invalidate prior review/approval/allowlist decision for the changed tool definition and rerun
    policy evaluation
  execution_check: Tool fingerprint at execution MUST equal the fingerprint bound to the approved/reviewed action.
  stale_decision_error: ExternalConflict
  canonicalization_standard: RFC-8785-JCS
  canonical_object_construction:
    all_listed_fields_are_present: true
    missing_optional_fields: encode_as_json_null
    unicode_normalization: none_preserve_code_points_as_required_by_JCS
    number_domain: I-JSON / IEEE-754 compatible JSON numbers only
    utf8_encoding_after_jcs: true
  cross_runtime_requirement: Python/Rust/TypeScript implementations MUST pass the same RFC-8785 golden vectors.
```

## A.memory.yaml

```yaml
schema_version: 1
layers:
  ephemeral: Current task scratch/context only.
  working: Recent project/thread context with expiration.
  durable: User-approved or strongly sourced stable work facts.
  relationship: Professional interaction preferences and context; no sensitive profiling unless explicitly allowed.
claim_lifecycle:
- candidate
- accepted
- superseded
- expired
- rejected
rules:
- source_required_for_operational_fact
- user_correction_supersedes_inference
- sensitive_personal_attributes_not_inferred
- memory_can_be_disabled_per_workspace
- deletion_propagates_to_indexes_subject_to_audit_retention_policy
```

## A.models.yaml

```yaml
schema_version: 1
model_router:
  providers: dynamic_registry
  supported_profile_types:
  - local_openai_compatible
  - openai
  - anthropic
  - azure_openai
  - google
  - custom_openai_compatible
  - mcp_sampling_if_policy_allows
  selection_order:
  - hard_privacy_rules
  - required_capabilities
  - data_residency
  - credential_health
  - user_preference
  - quality_fit
  - latency
  - cost
  fallback: explicit_and_auditable
credential_policy:
- keys_rotate_without_code_changes
- multiple_profiles_per_provider
- secrets_never_return_to_frontend_after_save
- provider_model_catalog_is_runtime_data_not_business_logic
```

## A.modes.yaml

```yaml
schema_version: 1
execution_modes:
  local_legacy:
    external_api_required: false
    mail_transport: outlook_classic_com
    llm: local_or_manual
    egress: policy_dependent
    purpose: Current bootstrap mode for locked-down Windows environments.
  connected_m365:
    external_api_required: true
    mail_transport: microsoft_graph
    channels:
    - teams
    - outlook
    - m365_copilot
    egress: tenant_and_policy_controlled
  connected_generic:
    external_api_required: true
    mail_transport: connector_defined
    channels:
    - desktop
    - web
    - third_party
    egress: connector_and_policy_controlled
  hybrid:
    external_api_required: optional
    mail_transport: best_available_connector
    llm: local_and_cloud
    egress: per_task_policy
  policy_auto:
    external_api_required: depends
    mail_transport: routed
    llm: routed
    egress: depends
    purpose: Select only among policy-compliant connectors and models.
privacy_classes:
  local_only: No payload leaves the local device except user-approved corporate-client actions already performed
    by that client.
  tenant_only: Data may move only inside approved organization/M365 tenant boundaries.
  controlled_external: External AI/API allowed only for data classes and providers explicitly approved by policy.
  external_allowed: External providers allowed subject to connector scopes, user policy and action approval.
```

## A.persistence.yaml

```yaml
schema_version: 1
engine: SQLite+FTS5 for local desktop baseline; repository interfaces allow enterprise store later.
tables:
- workspaces
- people
- organizations
- projects
- threads
- messages
- meetings
- commitments
- decisions
- work_items
- relationship_profiles
- memory_claims
- agent_tasks
- execution_plans
- actions
- runs
- approvals
- connector_profiles
- credential_profiles
- model_profiles
- scheduled_jobs
- artifacts
- events
- audit_records
- sync_cursors
- subscriptions
- settings
never_store_plaintext:
- api_keys
- oauth_refresh_tokens
- passwords
- private_keys
indexes:
- project_id
- thread_id
- person_id
- status
- due_at
- external_id
- created_at
- fts_user_content
migration_rules:
- versioned
- backup_before_write
- fixture_test_old_to_new
- fail_closed_on_migration_error
- no_silent_database_reset
store_profiles:
  local_authoritative:
    engine: SQLite+FTS5
    scope: single local workspace authority
  enterprise_authoritative_reference:
    engine: PostgreSQL
    scope: managed/headless Core; tenant deployment may substitute an equivalent transactional store through repository
      contracts
  rule: Exactly one authoritative mutable store per workspace. Local and managed stores may synchronize/cache but
    must not operate as independent multi-master authorities.
```

## A.product.yaml

```yaml
schema_version: 1
product:
  id: maia
  name: MAIA
  expanded_name: Multichannel Automation & Intelligent Assistance
  version: 1.2.0
  edition: Universal Executive Agent - Topology & Concurrency Hardening
  snapshot_date: '2026-09-11'
  promise: A universal, human-governed executive agent that understands communication, commitments, relationships
    and work context across mail, Teams, calendar, documents and tools.
principles:
- channel_neutral_core
- mail_is_a_strategic_first_class_domain_but_not_a_transport_lock_in
- human_is_highest_authority
- connector_and_model_independence
- local_first_when_required
- no_hidden_execution
- auditable_provenance
- least_privilege
- approval_before_material_risk
- canonical_spec_first
- future_m365_ready
non_goals:
- hard_dependency_on_outlook_classic
- hard_dependency_on_microsoft_graph
- single_model_lock_in
- secret_storage_in_sqlite
- silent_external_send
- silent_destructive_actions
- consumer_ai_dom_scraping
- security_bypass_to_reach_enterprise_data
```

## A.scheduler.yaml

```yaml
schema_version: 1
job_types:
- time_based
- recurring
- condition_watch
- follow_up_watch
rules:
- scheduled_execution_reuses_same_policy_gates_as_interactive_tasks
- send_external_never_becomes_auto_approved_only_because_job_is_scheduled
- condition_watch_emits_only_on_state_change_or_threshold
- jobs_have_owner_timezone
- missed_run_policy_is_explicit
examples:
- daily_priority_brief
- meeting_prebrief
- overdue_commitment_check
- awaited_reply_followup
- weekly_project_digest
```

## A.security.yaml

```yaml
schema_version: 1
protected_assets:
- mail_content
- teams_content
- calendar
- attachments
- credentials
- relationship_context
- workgraph
- audit_records
- policies
- connector_tokens
threats:
  prompt_injection_mail_or_attachment:
  - untrusted_content_boundary
  - instruction_source_labels
  - no_tool_call_from_untrusted_text_without_planner_validation
  - approval_rendering_allowlist
  - remote_resource_suppression
  wrong_recipient_or_reply_all:
  - recipient_diff
  - external_domain_warning
  - approval_hash
  secret_exfiltration:
  - os_keyring
  - redaction
  - frontend_non_return
  - no_secret_in_logs_db_exports
  connector_overprivilege:
  - least_scopes
  - capability_registry
  - admin_policy
  - periodic_scope_review
  silent_model_or_connector_swap:
  - requested_actual_provenance
  - no_hidden_fallback
  webhook_spoofing:
  - signature_or_token_validation
  - client_state
  - replay_window
  - idempotency
  ssrf_custom_endpoint:
  - scheme_allowlist
  - dns_recheck
  - link_local_block
  - https_remote_default
  malicious_mcp_tool:
  - allowlist
  - schema_validation
  - risk_class_mapping
  - sandbox
  - approval_gate
  - tool_definition_fingerprint
  - definition_change_invalidates_approval
  - server_identity_binding
  supply_chain:
  - signed_updates
  - lockfiles
  - dependency_audit
  - secret_scan
  data_retention_mismatch:
  - workspace_retention_policy
  - export_delete_runbooks
  - tenant_policy_override
  credential_compromise:
  - immediate_profile_suspend
  - provider_or_tenant_revoke
  - forced_rotation
  - invalidate_dependent_sessions_and_subscriptions
  - audit_since_last_known_good
  - blast_radius_review
  - incident_record
  concurrent_or_stale_approval:
  - compare_and_swap_version
  - action_hash_binding
  - first_terminal_decision_wins
  - ExternalConflict_on_stale_decision
  third_party_personal_data:
  - data_minimization
  - purpose_and_retention_policy
  - subject_linked_provenance
  - subject_request_workflow
  - sensitive_inference_block
  data_exfiltration_via_approval_rendering:
  - desktop_csp_blocks_remote_images_media_frames
  - teams_cards_generated_from_typed_fields_not_untrusted_card_json
  - no_untrusted_Image_Media_BackgroundImage_or_Action_OpenUrl_elements
  - escape_or_strip_markdown_links_in_untrusted_snippets
  - data_uri_blocked_in_approval_surfaces
  - trusted_static_assets_are_app_owned_or_tenant_allowlisted
  graph_poisoning_by_inference:
  - inferred_edges_are_explicitly_marked
  - source_ref_and_confidence_required
  - inferred_commitments_default_to_candidate
  - unconfirmed_inference_cannot_authorize_side_effects
  - ui_badge_suggested_until_confirmed_or_policy_promoted
invariants:
- human_authority
- least_privilege
- all_side_effects_are_auditable
- secrets_are_never_plaintext_persistent
- content_is_data_not_instruction
- enterprise_policy_can_be_stricter_than_user_policy
approval_rendering_policy:
  untrusted_content_rendering: plain_text_or_sanitized_text_only
  desktop:
    csp: default-src 'self'; img-src 'self'; media-src 'none'; frame-src 'none'; object-src 'none'
    block_data_uri: true
    external_navigation: explicit_user_action_only
  teams_adaptive_cards:
    payload_source: typed MAIA card schema only
    forbid_untrusted_elements:
    - Image
    - ImageSet
    - Media
    - BackgroundImage
    - Action.OpenUrl
    untrusted_markdown_links: escape_or_strip
    static_images: app_owned_or_tenant_allowlisted_only
  outlook_surface:
    same_typed_fields_and_sanitization: true
    remote_content_from_message_body_not_embedded_in_approval: true
```

## A.sources.snapshot.yaml

```yaml
schema_version: 1
snapshot_date: '2026-09-11'
sources:
- id: MS_AGENTS_SDK
  url: https://learn.microsoft.com/en-us/microsoft-365/agents-sdk/agents-sdk-overview
  purpose: Multichannel agent/channel abstraction and model-agnostic positioning.
- id: MS_TEAMS_AGENTS
  url: https://learn.microsoft.com/en-us/microsoftteams/platform/agents-in-teams/overview
  purpose: Teams agent surfaces and extension to Outlook/M365.
- id: MS_OUTLOOK_NEW
  url: https://learn.microsoft.com/en-us/office/dev/add-ins/outlook/one-outlook
  purpose: New Outlook does not support COM/VSTO; web add-ins are migration path.
- id: MS_GRAPH_MAIL_DELTA
  url: https://learn.microsoft.com/en-us/graph/delta-query-messages
  purpose: Incremental mail synchronization.
- id: MS_GRAPH_NOTIFICATIONS
  url: https://learn.microsoft.com/en-us/graph/api/resources/change-notifications-api-overview?view=graph-rest-1.0
  purpose: Change notification subscriptions for Outlook/Teams resources.
- id: MCP_2026_07_28
  url: https://blog.modelcontextprotocol.io/posts/2026-07-28/
  purpose: 'Current MCP protocol direction: stateless core, auth hardening, extensions/tasks.'
- id: VIKTOR_START
  url: https://viktor.com/docs/getting-started
  purpose: 'Benchmark: Slack/Teams coworker, broad integrations.'
- id: VIKTOR_SCHEDULED
  url: https://viktor.com/docs/scheduled-tasks
  purpose: 'Benchmark: conversational scheduling.'
- id: VIKTOR_API
  url: https://viktor.com/docs/public-api
  purpose: 'Benchmark: scoped API keys, asynchronous runs, MCP preference.'
- id: VIKTOR_TOOLS
  url: https://viktor.com/docs/connect-your-tools
  purpose: 'Benchmark: OAuth tool connections and MCP/custom APIs.'
- id: EU_GDPR_ART5_17
  url: https://eur-lex.europa.eu/legal-content/EN-PL/TXT/?uri=CELEX:32016R0679
  purpose: GDPR principles including purpose limitation/data minimisation and data-subject rectification/erasure
    rights.
- id: EDPB_DATA_SUBJECT_RIGHTS
  url: https://www.edpb.europa.eu/topics/key-gdpr-concepts/data-subject-rights_en
  purpose: Current EDPB overview of data-subject rights and controller procedures.
- id: RFC8785_JCS
  url: https://www.rfc-editor.org/rfc/rfc8785.html
  purpose: JSON Canonicalization Scheme used for deterministic MCP tool-definition fingerprints.
- id: MS_TEAMS_CORE_CONCEPTS
  url: https://learn.microsoft.com/en-us/microsoftteams/platform/teams-sdk/teams/core-concepts
  purpose: Teams/Bot routing requires an agent endpoint; local development uses DevTunnel.
- id: MS_AGENTS_DEPLOY
  url: https://learn.microsoft.com/en-us/microsoft-365/agents-sdk/deploy-azure-bot-service-manually
  purpose: Production Agents SDK agent is a deployed web application with a public messaging endpoint.
- id: MS_AZURE_RELAY
  url: https://learn.microsoft.com/en-us/azure/azure-relay/relay-what-is-it
  purpose: Hybrid Connections provide bidirectional HTTP/WebSocket relay without opening inbound firewall ports.
- id: MS_COM_MESSAGE_FILTER
  url: https://learn.microsoft.com/en-us/windows/win32/api/objidl/nf-objidl-imessagefilter-retryrejectedcall
  purpose: COM IMessageFilter retry semantics for SERVERCALL_RETRYLATER / SERVERCALL_REJECTED.
- id: MS_OFFICE_THREADING
  url: https://learn.microsoft.com/en-us/visualstudio/vsto/threading-support-in-office
  purpose: Office COM can reject calls while busy/modal; callers must handle/retry rejected calls.
- id: MS_GRAPH_MESSAGE_RESOURCE
  url: https://learn.microsoft.com/en-us/graph/api/resources/message?view=graph-rest-1.0
  purpose: Message changeKey is the version token; messages support delta/change notifications.
- id: MS_GRAPH_MESSAGE_SEND
  url: https://learn.microsoft.com/en-us/graph/api/message-send?view=graph-rest-1.0
  purpose: Send existing draft is POST and does not document an If-Match request header.
- id: MS_TEAMS_CARD_FORMAT
  url: https://learn.microsoft.com/en-us/microsoftteams/platform/task-modules-and-cards/cards/cards-format
  purpose: Teams Adaptive Cards support Markdown links; Markdown images are not supported, while explicit Image
    elements can reference URLs.
- id: ADAPTIVE_CARD_IMAGE
  url: https://adaptivecards.io/explorer/Image.html
  purpose: Adaptive Card Image.url supports URI and data URI in schema 1.2+, motivating typed element allowlisting
    for approval cards.
```

## A.states.yaml

```yaml
schema_version: 1
machines:
  AgentTaskState:
    states:
    - draft
    - preflight
    - awaiting_approval
    - queued
    - running
    - pausing
    - paused
    - partially_completed
    - completed
    - canceled
    - failed
    transitions:
      draft:
      - preflight
      - canceled
      preflight:
      - awaiting_approval
      - queued
      - failed
      - canceled
      awaiting_approval:
      - queued
      - canceled
      - failed
      queued:
      - running
      - canceled
      - failed
      running:
      - pausing
      - partially_completed
      - completed
      - failed
      - canceled
      paused:
      - queued
      - canceled
      partially_completed:
      - queued
      - completed
      - failed
      - canceled
      pausing:
      - paused
      - partially_completed
      - completed
      - failed
      - canceled
  ActionState:
    states:
    - planned
    - gated
    - awaiting_approval
    - queued
    - running
    - reconciling
    - retryable_error
    - completed
    - skipped
    - failed
    - canceled
    transitions:
      planned:
      - gated
      - canceled
      gated:
      - awaiting_approval
      - queued
      - skipped
      - failed
      awaiting_approval:
      - queued
      - canceled
      - failed
      queued:
      - running
      - canceled
      running:
      - reconciling
      - retryable_error
      - completed
      - failed
      - canceled
      retryable_error:
      - queued
      - failed
      - canceled
      reconciling:
      - completed
      - retryable_error
      - failed
      - canceled
  ApprovalState:
    states:
    - not_required
    - pending
    - approved
    - rejected
    - expired
    - revoked
    transitions:
      pending:
      - approved
      - rejected
      - expired
      - revoked
      approved:
      - revoked
  CommitmentStatus:
    states:
    - candidate
    - proposed
    - confirmed
    - in_progress
    - fulfilled
    - overdue
    - canceled
    - disputed
    transitions:
      candidate:
      - proposed
      - confirmed
      - canceled
      proposed:
      - confirmed
      - canceled
      - disputed
      confirmed:
      - in_progress
      - fulfilled
      - overdue
      - canceled
      - disputed
      in_progress:
      - fulfilled
      - overdue
      - canceled
      - disputed
      overdue:
      - fulfilled
      - canceled
      - disputed
  ConnectorHealth:
    states:
    - unconfigured
    - needs_auth
    - connecting
    - healthy
    - degraded
    - rate_limited
    - blocked
    - error
    transitions:
      unconfigured:
      - needs_auth
      - connecting
      - blocked
      needs_auth:
      - connecting
      - blocked
      - error
      - unconfigured
      connecting:
      - healthy
      - degraded
      - rate_limited
      - blocked
      - error
      - needs_auth
      - unconfigured
      healthy:
      - degraded
      - rate_limited
      - blocked
      - error
      - needs_auth
      - unconfigured
      degraded:
      - healthy
      - connecting
      - rate_limited
      - blocked
      - error
      - needs_auth
      - unconfigured
      rate_limited:
      - connecting
      - healthy
      - degraded
      - blocked
      - error
      - needs_auth
      - unconfigured
      blocked:
      - needs_auth
      - connecting
      - error
      - unconfigured
      error:
      - connecting
      - healthy
      - degraded
      - needs_auth
      - blocked
      - unconfigured
    semantics: observed_health_state_with_explicit_allowed_transitions
  RunState:
    states:
    - created
    - starting
    - running
    - retryable_error
    - outcome_unknown
    - reconciling
    - completed
    - failed
    - canceled
    transitions:
      created:
      - starting
      - canceled
      starting:
      - running
      - retryable_error
      - failed
      - canceled
      running:
      - retryable_error
      - outcome_unknown
      - completed
      - failed
      - canceled
      retryable_error:
      - starting
      - failed
      - canceled
      outcome_unknown:
      - reconciling
      reconciling:
      - completed
      - retryable_error
      - failed
    semantics: one execution attempt; outcome_unknown forbids blind retry until connector-specific reconciliation
invalid_transition: return InvalidStateTransition and write audit event; never repair silently
pause_semantics:
  model: quiescent_non_preemptive
  task_transition: running -> pausing -> paused
  on_pause_request:
  - stop_scheduling_new_actions_or_runs
  - request_cancel_only_for_in_flight_runs_whose_contract_is_cancellable
  - allow_non_cancellable_in_flight_runs_to_reach_terminal_or_outcome_unknown
  - enter_paused_only_when_no_in_flight_run_remains
  action_state_paused_is_intentionally_absent: true
  rationale: A generic Action pause would falsely promise preemption for non-cancellable side effects such as mail.send.
```

## A.sync.yaml

```yaml
schema_version: 1
strategies:
  outlook_classic_com: event_or_poll_based_best_effort with bounded scans and stable entry IDs when available
  microsoft_graph_mail: delta query per mail folder plus change notifications where deployment permits
  microsoft_graph_teams: change notifications/subscriptions plus targeted fetch
  calendar: delta/change notifications when available
delivery_semantics: at_least_once; deduplicate by connector + external_id + version/change token
idempotency:
- incoming_event_key
- action_idempotency_key
- webhook_replay_guard
- send_action_draft_hash
recovery:
- subscription_renewal
- lifecycle_notifications
- missed_notification_resync
- delta_token_reset_requires_bounded_full_resync
pre_side_effect_freshness:
  applies_to:
  - reply_or_forward_based_on_mutable_message
  - send_existing_draft
  - calendar_update_or_cancel
  - side_effect_using_mutable_external_object
  stored_tokens:
  - connector_external_id
  - source_version_token
  - normalized_payload_hash
  graph_mail_version_sources:
  - changeKey
  - '@odata.etag_when_returned'
  rule: Immediately before material side effect, re-read/conditionally validate the source version. If the connector
    does not document a strong If-Match precondition for that operation, perform compare-before-execute and fail
    closed on mismatch.
  on_mismatch:
  - invalidate_or_supersede_plan
  - rebuild_recipient_and_content_diff
  - recompute_action_hash
  - revoke_stale_approval
  - require_new_approval_if_gate_demands
  send_note: Microsoft Graph send-draft is POST and does not document If-Match on that operation; MAIA MUST NOT
    assume conditional-send support.
side_effect_reconciliation:
  rule: A timeout/disconnect after a non-idempotent side effect may create outcome_unknown. Do not automatically
    retry until connector-specific reconciliation proves whether the effect happened.
  recommended_send_marker: Where connector permits, stamp a stable MAIA action/idempotency marker on the draft before
    send and search/reconcile by that marker after ambiguous outcomes.
  action_state: reconciling
  run_state: outcome_unknown -> reconciling
```

## A.testing.yaml

```yaml
schema_version: 1
suites:
  spec:
  - schema_valid
  - unique_ids
  - state_transitions
  - acceptance_refs
  - forbidden_secret_fields
  - generated_drift
  - risk_enum_integrity
  - dynamic_risk_classifier_refs
  - approval_gate_state_mapping
  - all_fsm_transitions_explicit
  - mcp_rfc8785_declaration
  - pause_semantics
  - teams_store_authority_topology
  - approval_mutation_policy
  - transport_envelope_contract
  core:
  - planner
  - approval_gate
  - privacy_gate
  - model_router
  - connector_router
  - workgraph
  - commitments
  - memory
  - scheduler
  - idempotency
  - approval_concurrency
  - approval_action_hash_recheck
  - quiescent_pause
  - run_outcome_unknown_reconciliation
  - source_freshness_precondition
  - inference_cannot_authorize_side_effect
  connector_contract:
  - capability_discovery
  - health
  - cancellation
  - timeouts
  - error_mapping
  - provenance
  - auth_failure
  - rate_limit
  - outlook_com_sta
  - outlook_com_busy_retry
  - source_version_recheck
  - ambiguous_send_reconciliation
  e2e:
  - classic_read_to_draft
  - teams_request_to_approval
  - graph_delta_resync
  - meeting_prebrief
  - overdue_commitment
  - api_key_rotation
  - mcp_read_tool
  security:
  - prompt_injection_mail
  - recipient_swap
  - reply_all
  - ssrf
  - webhook_replay
  - malicious_mcp
  - secret_grep
  - archive_bomb
  - policy_bypass
  - mcp_tool_definition_rug_pull
  - credential_compromise_response
  - stale_approval_decision
  - approval_remote_resource_exfiltration
  - unsafe_adaptive_card_element
  - approval_fatigue_grant_scope
  migration:
  - classic_to_graph
  - schema_upgrade
  - credential_reference_remap
golden_vectors:
- untrusted_mail_cannot_issue_tool_command
- reply_all_external_delta_forces_confirmation
- send_action_hash_change_invalidates_approval
- connector_fallback_identity_is_visible
- memory_claim_requires_source
- overdue_commitment_keeps_source
- tenant_only_blocks_external_ai
- mcp_tool_cannot_bypass_approval
- duplicate_webhook_does_not_duplicate_action
- mcp_tool_definition_change_invalidates_approval
- second_approval_decision_returns_external_conflict
- legacy_calendar_write_rejected_by_capability_gate
- core_has_no_direct_com_dependency
- task_pause_waits_for_non_cancellable_run_before_paused
- mcp_rfc8785_fingerprint_vector_is_cross_runtime_stable
- mcp_missing_optional_fingerprint_fields_are_null
- teams_cloud_adapter_never_opens_local_sqlite
- payload_mutation_revokes_old_approval_and_requires_new_version
- untrusted_card_content_cannot_load_remote_image_or_data_uri
- inferred_workgraph_edge_is_suggested_and_cannot_trigger_send
- graph_change_key_mismatch_replans_before_send
- ambiguous_send_is_reconciled_before_retry
- polish_hedged_commitment_stays_candidate
```

## A.transport.yaml

```yaml
schema_version: 1
principles:
- single_state_authority_per_workspace
- channel_service_is_not_a_second_brain
- no_direct_cloud_sqlite_access
- outbound_only_local_relay_when_local_authority_is_used
- authenticated_envelopes
- at_least_once_delivery_with_idempotency
topologies:
  desktop_only:
    channel_endpoint: none
    core: local
    state_authority: sqlite
    public_inbound_required: false
  local_with_teams_relay:
    channel_endpoint: managed_https_agent_service
    core: local
    state_authority: sqlite
    relay: outbound_bidirectional_wss
    reference: azure_relay_hybrid_connections
    public_inbound_to_workstation: false
  enterprise_managed:
    channel_endpoint: managed_https_agent_service
    core: managed_headless
    state_authority: postgresql_reference
    desktop: thin_client_or_capability_worker
  hybrid_enterprise:
    channel_endpoint: managed_https_agent_service
    core: managed_plus_local_worker
    state_authority: one_explicit_authority_per_workspace
    relay: outbound_authenticated_channel
envelope:
  required_fields:
  - envelope_id
  - workspace_id
  - task_or_approval_id
  - kind
  - actor_id
  - origin_surface
  - created_at
  - expires_at
  - nonce
  - payload_hash
  - payload
  - auth_context
  security:
  - short_lived_auth
  - replay_guard
  - workspace_binding
  - actor_binding
  - payload_hash_validation
  - idempotency_key
  approval_rules:
  - relay_cannot_mutate_action_payload
  - approval_decision_still_uses_CAS_in_authoritative_core
  - stale_relay_message_returns_ExternalConflict
offline_behavior:
  local_core_unreachable: channel service may queue bounded envelopes but may not execute local-authority side effects
  expiry: expired approval/action envelopes fail closed and require refresh
```

## A.ux.yaml

```yaml
schema_version: 1
zones:
  command_center:
  - today_brief
  - priority_inbox
  - commitments
  - meetings
  - awaiting_approvals
  conversation:
  - chat_with_maia
  - task_trace
  - sources
  - approval_cards
  mail:
  - thread_summary
  - why_it_matters
  - draft
  - commitments
  - relationship_context
  workgraph:
  - people
  - projects
  - threads
  - decisions
  - commitments
  - timeline
  settings:
  - connectors
  - models
  - credentials
  - policies
  - memory
  - scheduler
  - mcp
  - audit
surfaces:
- desktop
- teams
- outlook_addin
- m365_copilot_future
- api_mcp
async_states:
- idle
- loading
- success
- error
- canceled
- awaiting_approval
rules:
- same_task_identity_across_surfaces
- approval_card_shows_exact_side_effect
- no_color_only_status
- keyboard_accessible
- dark_light_ready
```

## A.workgraph.yaml

```yaml
schema_version: 1
node_types:
- person
- organization
- project
- thread
- message
- meeting
- commitment
- decision
- work_item
- artifact
edge_types:
- works_for
- participates_in
- belongs_to_project
- replies_to
- mentions
- depends_on
- promised_to
- decided_in
- follow_up_to
- supersedes
- attached_to
- scheduled_for
rules:
- edges_keep_source_and_confidence
- inference_edges_are_distinct_from_explicit_edges
- project_linking_can_be_corrected_by_user
- deleted_external_content_does_not_silently_delete_audit_history
use_cases:
- meeting_briefing
- thread_context
- commitment_tracking
- relationship_brief
- project_status
- contradiction_detection
edge_provenance:
  required_fields:
  - source_ref
  - confidence
  - is_inferred
  - status
  inferred_default:
    is_inferred: true
    status: candidate
    ui_badge: suggested_by_ai
    may_authorize_side_effect: false
  promotion: human confirmation or explicit policy-controlled deterministic evidence may promote; promotion is audited
  correction: user correction supersedes inference and triggers recomputation of derived context
```

# ZAŁĄCZNIK B - ADR-y

## B.ADR-0001

# ADR-0001: Desktop-first core, channel-neutral surfaces

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
MAIA Core is independent of Outlook/Teams UI; surfaces are adapters.

## Decision
Build the durable product around a desktop/service core and channel gateways rather than an Outlook plug-in.

## Alternatives considered
- Embed the behavior directly in the current client-specific implementation.
- Allow each feature to make its own local decision.

## Why alternatives were rejected
They create duplicated policy, lock-in or behavior that cannot be audited consistently across surfaces.

## Consequences
Avoids migration lock-in and preserves local/enterprise deployment options.

## Security / privacy impact
The implementation must continue to satisfy `spec/security.yaml`, `spec/approval.yaml`, and the workspace policy profile.

## Migration impact
Existing prototype behavior is preserved only when compatible with this ADR. Incompatible implementation details migrate behind the new contract.

## B.ADR-0002

# ADR-0002: Outlook Classic is a bootstrap connector, not the core

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
Current COM automation is useful today but unsupported in new Outlook.

## Decision
Keep TrustedBridge as OutlookClassicConnector behind the shared connector contract.

## Alternatives considered
- Embed the behavior directly in the current client-specific implementation.
- Allow each feature to make its own local decision.

## Why alternatives were rejected
They create duplicated policy, lock-in or behavior that cannot be audited consistently across surfaces.

## Consequences
Existing work is preserved while future migration becomes a connector swap.

## Security / privacy impact
The implementation must continue to satisfy `spec/security.yaml`, `spec/approval.yaml`, and the workspace policy profile.

## Migration impact
Existing prototype behavior is preserved only when compatible with this ADR. Incompatible implementation details migrate behind the new contract.

## B.ADR-0003

# ADR-0003: Microsoft Graph is the primary future M365 data connector

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
New Outlook removes COM/VSTO extensibility.

## Decision
Use Graph for durable mail/calendar/files data access when tenant policy allows.

## Alternatives considered
- Embed the behavior directly in the current client-specific implementation.
- Allow each feature to make its own local decision.

## Why alternatives were rejected
They create duplicated policy, lock-in or behavior that cannot be audited consistently across surfaces.

## Consequences
Requires Entra app/permissions but works beyond one desktop client.

## Security / privacy impact
The implementation must continue to satisfy `spec/security.yaml`, `spec/approval.yaml`, and the workspace policy profile.

## Migration impact
Existing prototype behavior is preserved only when compatible with this ADR. Incompatible implementation details migrate behind the new contract.

## B.ADR-0004

# ADR-0004: Teams is a first-class MAIA surface with explicit channel topology

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
Teams must be a first-class MAIA surface, but a Teams/M365 agent is delivered to a network endpoint while the personal/local MAIA state may be held only in a workstation SQLite database behind NAT/firewalls. A cloud channel handler cannot directly open that local file.

## Decision
Expose the same MAIA task runtime through Teams/M365 channel abstractions, but separate the **Channel Adapter** from the authoritative Core/store. Production Teams traffic terminates at a managed public HTTPS agent endpoint. If a workspace is local-authoritative, the managed Channel Adapter exchanges authenticated, replay-protected task/approval envelopes with the local Core through an **outbound-only bidirectional relay**. The reference pattern is Azure Relay Hybrid Connections (or a tenant-approved equivalent). No inbound workstation port is required and the cloud adapter never opens local SQLite.

For managed enterprise workspaces, a headless Core may run beside the Channel Adapter and use a managed transactional store (PostgreSQL is the reference profile). A workspace has exactly one mutable state authority; hybrid deployment must not become multi-master.

Dev Tunnel is permitted for development/testing only and is not the production architecture.

## Alternatives considered
- Expose the desktop runtime directly through a public tunnel in production.
- Let the Teams service access or synchronize the SQLite file directly.
- Run a separate Teams-specific brain with its own state.

## Why alternatives were rejected
They create unacceptable firewall/NAT assumptions, split-brain state, fragile file synchronization, or duplicated policy/orchestration.

## Consequences
The product gains an explicit transport/service boundary and some deployment complexity. It also becomes possible to support Teams without making the desktop machine a public server or making SQLite a cloud-shared database.

## Security / privacy impact
Relay envelopes are authenticated, workspace/actor bound, replay-protected, idempotent and short-lived. Approval still resolves via CAS in the authoritative Core. Channel transport never broadens connector/model permissions.

## Migration impact
Existing desktop-only use remains valid. Teams is enabled only when a compliant channel topology is configured.

## B.ADR-0005

# ADR-0005: Human authority and approval hashing

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
Material side effects must remain under user control.

## Decision
Approval binds to exact action payload hash; changed recipients/content invalidate approval.

## Alternatives considered
- Embed the behavior directly in the current client-specific implementation.
- Allow each feature to make its own local decision.

## Why alternatives were rejected
They create duplicated policy, lock-in or behavior that cannot be audited consistently across surfaces.

## Consequences
Prevents stale or ambiguous approvals.

## Security / privacy impact
The implementation must continue to satisfy `spec/security.yaml`, `spec/approval.yaml`, and the workspace policy profile.

## Migration impact
Existing prototype behavior is preserved only when compatible with this ADR. Incompatible implementation details migrate behind the new contract.

## B.ADR-0006

# ADR-0006: Canonical machine-readable specification

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
Narrative drift is unacceptable for an agent that can act.

## Decision
Enums, states, policies, IPC, persistence and acceptance live in spec/*.yaml.

## Alternatives considered
- Embed the behavior directly in the current client-specific implementation.
- Allow each feature to make its own local decision.

## Why alternatives were rejected
They create duplicated policy, lock-in or behavior that cannot be audited consistently across surfaces.

## Consequences
Generators/tests become required build gates.

## Security / privacy impact
The implementation must continue to satisfy `spec/security.yaml`, `spec/approval.yaml`, and the workspace policy profile.

## Migration impact
Existing prototype behavior is preserved only when compatible with this ADR. Incompatible implementation details migrate behind the new contract.

## B.ADR-0007

# ADR-0007: Task/Plan/Action/Run separation

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
Retries and multi-step execution must remain auditable.

## Decision
Task is intent; Plan is proposed steps; Action is logical side effect; Run is one execution attempt.

## Alternatives considered
- Embed the behavior directly in the current client-specific implementation.
- Allow each feature to make its own local decision.

## Why alternatives were rejected
They create duplicated policy, lock-in or behavior that cannot be audited consistently across surfaces.

## Consequences
Allows retry/fallback without corrupting logical history.

## Security / privacy impact
The implementation must continue to satisfy `spec/security.yaml`, `spec/approval.yaml`, and the workspace policy profile.

## Migration impact
Existing prototype behavior is preserved only when compatible with this ADR. Incompatible implementation details migrate behind the new contract.

## B.ADR-0008

# ADR-0008: No hidden model or connector fallback

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
Silent substitution undermines trust and can change privacy/cost/side effects.

## Decision
Persist requested and actual model/connector; pre-approve material differences.

## Alternatives considered
- Embed the behavior directly in the current client-specific implementation.
- Allow each feature to make its own local decision.

## Why alternatives were rejected
They create duplicated policy, lock-in or behavior that cannot be audited consistently across surfaces.

## Consequences
Routing becomes transparent and testable.

## Security / privacy impact
The implementation must continue to satisfy `spec/security.yaml`, `spec/approval.yaml`, and the workspace policy profile.

## Migration impact
Existing prototype behavior is preserved only when compatible with this ADR. Incompatible implementation details migrate behind the new contract.

## B.ADR-0009

# ADR-0009: WorkGraph as the long-term memory backbone

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
Mail-only memory cannot represent projects, people, meetings and commitments.

## Decision
Use a source-linked graph over normalized domain entities.

## Alternatives considered
- Embed the behavior directly in the current client-specific implementation.
- Allow each feature to make its own local decision.

## Why alternatives were rejected
They create duplicated policy, lock-in or behavior that cannot be audited consistently across surfaces.

## Consequences
Enables cross-channel context and contradiction detection.

## Security / privacy impact
The implementation must continue to satisfy `spec/security.yaml`, `spec/approval.yaml`, and the workspace policy profile.

## Migration impact
Existing prototype behavior is preserved only when compatible with this ADR. Incompatible implementation details migrate behind the new contract.

## B.ADR-0010

# ADR-0010: Commitments are explicit entities

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
Promises and due dates are central to executive assistance.

## Decision
Extract candidate commitments with source/confidence; confirmation policy controls promotion.

## Alternatives considered
- Embed the behavior directly in the current client-specific implementation.
- Allow each feature to make its own local decision.

## Why alternatives were rejected
They create duplicated policy, lock-in or behavior that cannot be audited consistently across surfaces.

## Consequences
MAIA can track obligations without pretending inference is fact.

## Security / privacy impact
The implementation must continue to satisfy `spec/security.yaml`, `spec/approval.yaml`, and the workspace policy profile.

## Migration impact
Existing prototype behavior is preserved only when compatible with this ADR. Incompatible implementation details migrate behind the new contract.

## B.ADR-0011

# ADR-0011: Professional Relationship Intelligence only

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
Useful tone/context must not become uncontrolled personal profiling.

## Decision
Store work-relevant communication preferences and project context with sources; do not infer sensitive traits.

## Alternatives considered
- Embed the behavior directly in the current client-specific implementation.
- Allow each feature to make its own local decision.

## Why alternatives were rejected
They create duplicated policy, lock-in or behavior that cannot be audited consistently across surfaces.

## Consequences
Improves drafting while reducing privacy risk.

## Security / privacy impact
The implementation must continue to satisfy `spec/security.yaml`, `spec/approval.yaml`, and the workspace policy profile.

## Migration impact
Existing prototype behavior is preserved only when compatible with this ADR. Incompatible implementation details migrate behind the new contract.

## B.ADR-0012

# ADR-0012: OS-native secret store

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
API keys and OAuth tokens are high-value secrets.

## Decision
Persist only secret references/metadata in SQLite; use OS keyring/enterprise vault for secret material.

## Alternatives considered
- Embed the behavior directly in the current client-specific implementation.
- Allow each feature to make its own local decision.

## Why alternatives were rejected
They create duplicated policy, lock-in or behavior that cannot be audited consistently across surfaces.

## Consequences
Secret lifecycle becomes independent from project export/backups.

## Security / privacy impact
The implementation must continue to satisfy `spec/security.yaml`, `spec/approval.yaml`, and the workspace policy profile.

## Migration impact
Existing prototype behavior is preserved only when compatible with this ADR. Incompatible implementation details migrate behind the new contract.

## B.ADR-0013

# ADR-0013: Provider/model registry is dynamic

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
Model IDs, prices and capabilities change quickly.

## Decision
Treat provider catalogs as dated runtime/snapshot data.

## Alternatives considered
- Embed the behavior directly in the current client-specific implementation.
- Allow each feature to make its own local decision.

## Why alternatives were rejected
They create duplicated policy, lock-in or behavior that cannot be audited consistently across surfaces.

## Consequences
No hard-coded business logic around current model names.

## Security / privacy impact
The implementation must continue to satisfy `spec/security.yaml`, `spec/approval.yaml`, and the workspace policy profile.

## Migration impact
Existing prototype behavior is preserved only when compatible with this ADR. Incompatible implementation details migrate behind the new contract.

## B.ADR-0014

# ADR-0014: MCP client and server are first-class

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
Breadth of integrations should not require bespoke connectors for everything.

## Decision
Implement scoped MCP client and a conservative read-mostly MAIA MCP server.

## Alternatives considered
- Embed the behavior directly in the current client-specific implementation.
- Allow each feature to make its own local decision.

## Why alternatives were rejected
They create duplicated policy, lock-in or behavior that cannot be audited consistently across surfaces.

## Consequences
Extensibility grows while approval/policy remain centralized.

## Security / privacy impact
The implementation must continue to satisfy `spec/security.yaml`, `spec/approval.yaml`, and the workspace policy profile.

## Migration impact
Existing prototype behavior is preserved only when compatible with this ADR. Incompatible implementation details migrate behind the new contract.

## B.ADR-0015

# ADR-0015: Untrusted communication content cannot issue instructions

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
Email/Teams/attachments are adversarial input surfaces.

## Decision
Tag source trust and never promote content instructions into system/tool authority.

## Alternatives considered
- Embed the behavior directly in the current client-specific implementation.
- Allow each feature to make its own local decision.

## Why alternatives were rejected
They create duplicated policy, lock-in or behavior that cannot be audited consistently across surfaces.

## Consequences
Prompt injection becomes an explicit threat model concern.

## Security / privacy impact
The implementation must continue to satisfy `spec/security.yaml`, `spec/approval.yaml`, and the workspace policy profile.

## Migration impact
Existing prototype behavior is preserved only when compatible with this ADR. Incompatible implementation details migrate behind the new contract.

## B.ADR-0016

# ADR-0016: At-least-once event processing with idempotency

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
Graph/webhooks/desktop events may duplicate or arrive late.

## Decision
Deduplicate with stable event/action keys and persist cursors/subscription state.

## Alternatives considered
- Embed the behavior directly in the current client-specific implementation.
- Allow each feature to make its own local decision.

## Why alternatives were rejected
They create duplicated policy, lock-in or behavior that cannot be audited consistently across surfaces.

## Consequences
Reliable sync without duplicate sends/actions.

## Security / privacy impact
The implementation must continue to satisfy `spec/security.yaml`, `spec/approval.yaml`, and the workspace policy profile.

## Migration impact
Existing prototype behavior is preserved only when compatible with this ADR. Incompatible implementation details migrate behind the new contract.

## B.ADR-0017

# ADR-0017: Scheduler reuses the same gates

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
Automation must not become a bypass around human control.

## Decision
Scheduled/condition tasks run through privacy, capability and approval policies.

## Alternatives considered
- Embed the behavior directly in the current client-specific implementation.
- Allow each feature to make its own local decision.

## Why alternatives were rejected
They create duplicated policy, lock-in or behavior that cannot be audited consistently across surfaces.

## Consequences
Proactivity stays safe and predictable.

## Security / privacy impact
The implementation must continue to satisfy `spec/security.yaml`, `spec/approval.yaml`, and the workspace policy profile.

## Migration impact
Existing prototype behavior is preserved only when compatible with this ADR. Incompatible implementation details migrate behind the new contract.

## B.ADR-0018

# ADR-0018: Enterprise policy may only tighten user policy

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
Corporate deployments need central governance.

## Decision
Tenant/admin policy can restrict connectors, models, scopes and retention but cannot silently broaden user-authorized egress.

## Alternatives considered
- Embed the behavior directly in the current client-specific implementation.
- Allow each feature to make its own local decision.

## Why alternatives were rejected
They create duplicated policy, lock-in or behavior that cannot be audited consistently across surfaces.

## Consequences
Supports enterprise rollout without ambiguous authority.

## Security / privacy impact
The implementation must continue to satisfy `spec/security.yaml`, `spec/approval.yaml`, and the workspace policy profile.

## Migration impact
Existing prototype behavior is preserved only when compatible with this ADR. Incompatible implementation details migrate behind the new contract.

## B.ADR-0019

# ADR-0019: Signed updates and migration preflight

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
Agent updates can alter access and behavior.

## Decision
Require signed artifacts, schema preflight, backup and rollback metadata.

## Alternatives considered
- Embed the behavior directly in the current client-specific implementation.
- Allow each feature to make its own local decision.

## Why alternatives were rejected
They create duplicated policy, lock-in or behavior that cannot be audited consistently across surfaces.

## Consequences
Reduces supply-chain and destructive migration risk.

## Security / privacy impact
The implementation must continue to satisfy `spec/security.yaml`, `spec/approval.yaml`, and the workspace policy profile.

## Migration impact
Existing prototype behavior is preserved only when compatible with this ADR. Incompatible implementation details migrate behind the new contract.

## B.ADR-0020

# ADR-0020: No consumer AI DOM scraping

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
Browser automation against consumer AI UIs is fragile and policy-sensitive.

## Decision
Use official APIs, local endpoints, MCP or explicit manual handoff only.

## Alternatives considered
- Embed the behavior directly in the current client-specific implementation.
- Allow each feature to make its own local decision.

## Why alternatives were rejected
They create duplicated policy, lock-in or behavior that cannot be audited consistently across surfaces.

## Consequences
Keeps MAIA supportable and auditable.

## Security / privacy impact
The implementation must continue to satisfy `spec/security.yaml`, `spec/approval.yaml`, and the workspace policy profile.

## Migration impact
Existing prototype behavior is preserved only when compatible with this ADR. Incompatible implementation details migrate behind the new contract.

## B.ADR-0021

# ADR-0021: Multichannel product identity

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
The product is channel-neutral but the historical expansion "Mail Automation" over-positions one connector/domain.

## Decision
Expand MAIA as "Multichannel Automation & Intelligent Assistance". Mail remains a strategic first-class intelligence domain, not the transport/core identity.

## Alternatives considered
- Keep v1.0 behavior implicit.
- Let each surface/connector decide independently.

## Why alternatives were rejected
They reintroduce ambiguity, platform lock-in, drift or unauditable security behavior.

## Consequences
Brand and architecture now tell the same story; historical references may mention the earlier expansion only as legacy.

## Security / privacy impact
Implementation must satisfy `spec/security.yaml`, `spec/approval.yaml` and, where applicable, `spec/compliance.yaml`.

## Migration impact
This ADR is part of the v1.1 contract correction. Existing v1.0 implementations must migrate rather than preserve contradictory behavior.

## B.ADR-0022

# ADR-0022: OS-neutral Core with Windows reference bootstrap

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
The first working connector is Outlook Classic COM and therefore Windows-specific, while Teams/Graph/MCP are not.

## Decision
Keep canonical Core/domain/orchestrator/policy/connector contracts OS-neutral. Confine COM/Win32 dependencies to adapters. Windows is the reference Alpha desktop, not a universal architecture requirement.

## Alternatives considered
- Keep v1.0 behavior implicit.
- Let each surface/connector decide independently.

## Why alternatives were rejected
They reintroduce ambiguity, platform lock-in, drift or unauditable security behavior.

## Consequences
Enables later mixed-fleet deployment without promising macOS/Linux desktop support before a support-matrix decision.

## Security / privacy impact
Implementation must satisfy `spec/security.yaml`, `spec/approval.yaml` and, where applicable, `spec/compliance.yaml`.

## Migration impact
This ADR is part of the v1.1 contract correction. Existing v1.0 implementations must migrate rather than preserve contradictory behavior.

## B.ADR-0023

# ADR-0023: MCP tool-definition pinning uses RFC 8785 JCS

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
MCP tool catalogs are remotely supplied and can change after initial review, creating rug-pull/tool-poisoning risk. A vague `sha256(canonical_json)` rule is not interoperable across Python, Rust and TypeScript because serializers can differ in key ordering, number formatting and escaping.

## Decision
Bind tool trust and any approval to SHA-256 over the UTF-8 bytes of an **RFC 8785 JSON Canonicalization Scheme (JCS)** representation. The fingerprint object contains the canonical fields listed in `spec/mcp.yaml`; missing optional fields are represented as explicit JSON `null` so omission/null cannot silently change semantics. JCS does not perform Unicode normalization; code points are preserved as required by RFC 8785.

Tool definition change invalidates the prior review/approval and reruns policy. Immediately before execution, the discovered tool fingerprint must equal the fingerprint bound to the action/approval.

All supported runtime implementations must pass the same RFC-8785 golden vectors.

## Alternatives considered
- Language-native JSON serialization.
- Sorting keys in each runtime without defining number/string rules.
- Trusting only MCP `notifications/tools/list_changed`.

## Why alternatives were rejected
They allow cross-runtime hash drift or rely on a remote server to honestly announce a definition change.

## Consequences
A stable security fingerprint becomes portable across runtimes, at the cost of a small canonicalization dependency/conformance suite.

## Security / privacy impact
Reduces MCP rug-pull and stale-approval risk.

## Migration impact
Any v1.1 stored MCP tool fingerprint must be recomputed under RFC 8785 before trust/approval reuse.

## B.ADR-0024

# ADR-0024: Approval concurrency is optimistic and versioned

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
The same approval can be displayed on multiple surfaces and devices. Concurrent decisions must not race into ambiguous side effects.

## Decision
Use compare-and-swap on approval id, action hash, expected version and pending state. First terminal decision wins; stale/second attempts return ExternalConflict.

## Alternatives considered
- Keep v1.0 behavior implicit.
- Let each surface/connector decide independently.

## Why alternatives were rejected
They reintroduce ambiguity, platform lock-in, drift or unauditable security behavior.

## Consequences
Desktop/Teams remain consistent; material edits invalidate old approval and create a new version.

## Security / privacy impact
Implementation must satisfy `spec/security.yaml`, `spec/approval.yaml` and, where applicable, `spec/compliance.yaml`.

## Migration impact
This ADR is part of the v1.1 contract correction. Existing v1.0 implementations must migrate rather than preserve contradictory behavior.

## B.ADR-0025

# ADR-0025: Third-party personal-data governance uses policy-driven erasure and safe tombstones

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
WorkGraph and Relationship Intelligence store data about correspondents and meeting participants who are not MAIA users. Hard-deleting a `Person` can break references and audit integrity, while retaining identifying fields may violate an applicable erasure/suppression decision. A deterministic hash of an email/name is not a safe anonymization strategy and can remain personal data.

## Decision
Provide subject-linked locate/export/rectify/restrict/erase-or-pseudonymize workflows. When a structural record must remain, replace direct identifiers with a **cryptographically random opaque tombstone ID**, not a hash of PII; purge/suppress derived search indexes, embeddings and memory claims; and preserve only those edges/audit facts that remain necessary and permitted by policy/law. A linkable tombstone is explicitly treated as pseudonymized personal data, not anonymous data.

## Alternatives considered
- Unconditional hard delete of graph nodes.
- `SHA256(email)` tombstone identifiers.
- Preserve every edge regardless of re-identification risk.

## Why alternatives were rejected
They either damage integrity or create avoidable privacy/re-identification risk.

## Consequences
Erasure becomes a governed multi-store operation rather than a single SQL DELETE.

## Security / privacy impact
Improves data-subject handling while keeping legally/policy-required audit evidence segregated and minimal.

## Migration impact
Existing deletion workflows must purge derived data and migrate deterministic tombstones if any exist.

## B.ADR-0026

# ADR-0026: Task pause is quiescent, not Action preemption

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
A task can contain non-cancellable side effects such as mail.send. A generic ActionState.paused would falsely imply the runtime can suspend an irreversible external operation mid-call.

## Decision
Add AgentTaskState.pausing. A pause request stops scheduling new work, cancels only cancellable in-flight Runs, lets non-cancellable Runs drain or enter outcome_unknown/reconciliation, and enters paused only when no Run remains in flight. ActionState has no generic paused state.

## Alternatives considered
- Preserve the v1.1 implicit behavior.
- Let each connector/surface decide independently.

## Why alternatives were rejected
They make behavior ambiguous, unauditable, or unsafe across connectors.

## Consequences
Pause semantics become honest and testable across connectors.

## Security / privacy impact
Implementation must satisfy canonical security, approval, transport and compliance policy.

## Migration impact
This decision applies to v1.2+ implementations and supersedes contradictory v1.1 narrative assumptions.

## B.ADR-0027

# ADR-0027: Non-idempotent side effects require outcome reconciliation

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
A network/COM timeout after a send can leave the system unable to know whether the side effect happened. Blind retry can duplicate mail or other actions.

## Decision
Add RunState outcome_unknown/reconciling and ActionState reconciling. No automatic retry of a non-idempotent action occurs until the connector reconciles authoritative external state. Use a stable MAIA action marker where supported.

## Alternatives considered
- Preserve the v1.1 implicit behavior.
- Let each connector/surface decide independently.

## Why alternatives were rejected
They make behavior ambiguous, unauditable, or unsafe across connectors.

## Consequences
Prevents duplicate side effects at the cost of connector-specific reconciliation logic.

## Security / privacy impact
Implementation must satisfy canonical security, approval, transport and compliance policy.

## Migration impact
This decision applies to v1.2+ implementations and supersedes contradictory v1.1 narrative assumptions.

## B.ADR-0028

# ADR-0028: Mutable external source freshness is a precondition

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
Delta/webhook delivery is at-least-once and can lag. A draft/reply planned against stale recipients or thread state must not execute merely because an older approval exists.

## Decision
Persist source version tokens and re-check mutable source state immediately before material side effects. Prefer strong connector preconditions when documented; otherwise re-fetch and compare. Version mismatch revokes stale approval and triggers re-plan.

## Alternatives considered
- Preserve the v1.1 implicit behavior.
- Let each connector/surface decide independently.

## Why alternatives were rejected
They make behavior ambiguous, unauditable, or unsafe across connectors.

## Consequences
Reduces stale-thread and recipient races.

## Security / privacy impact
Implementation must satisfy canonical security, approval, transport and compliance policy.

## Migration impact
This decision applies to v1.2+ implementations and supersedes contradictory v1.1 narrative assumptions.

## B.ADR-0029

# ADR-0029: Approval rendering is typed and remote-resource safe

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
Untrusted mail/document text shown in desktop/Teams approval UI can contain markup/links designed to trigger remote fetches or confuse the approver. Teams host CSP is not controlled by MAIA.

## Decision
Generate approval surfaces only from typed MAIA fields. Desktop applies restrictive CSP and sanitization. Teams Adaptive Cards forbid untrusted Image/Media/BackgroundImage/Action.OpenUrl elements and strip/escape untrusted links; only app-owned/tenant-allowlisted static assets are allowed.

## Alternatives considered
- Preserve the v1.1 implicit behavior.
- Let each connector/surface decide independently.

## Why alternatives were rejected
They make behavior ambiguous, unauditable, or unsafe across connectors.

## Consequences
Blocks a rendering-based exfiltration path without claiming control over the Teams host CSP.

## Security / privacy impact
Implementation must satisfy canonical security, approval, transport and compliance policy.

## Migration impact
This decision applies to v1.2+ implementations and supersedes contradictory v1.1 narrative assumptions.

## B.ADR-0030

# ADR-0030: Approval-fatigue mitigation never learns authority from relationships

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
Frequent confirmations can cause rubber-stamping, but Relationship Intelligence is descriptive context, not an authorization system.

## Decision
Reduce prompts only through explicit, scoped and expiring durable grants or approved templates bound to action/recipient/template constraints. Relationship familiarity alone never grants execution authority, and external sends remain non-autonomous by default.

## Alternatives considered
- Preserve the v1.1 implicit behavior.
- Let each connector/surface decide independently.

## Why alternatives were rejected
They make behavior ambiguous, unauditable, or unsafe across connectors.

## Consequences
Improves usability without turning behavioral familiarity into permission.

## Security / privacy impact
Implementation must satisfy canonical security, approval, transport and compliance policy.

## Migration impact
This decision applies to v1.2+ implementations and supersedes contradictory v1.1 narrative assumptions.

# ZAŁĄCZNIK C - Runbooki

## C.approval-incident.md

# Runbook: Ambiguous or risky action

1. Stop before the side effect.
2. Show exact recipients/target, content summary, attachments, connector and risk class.
3. Explain why approval is required and any external-domain or destructive effect.
4. Hash the approved payload.
5. Execute only if the payload hash still matches.
6. If payload materially changes, invalidate approval and request again.
7. Persist the decision and actual execution result in the audit trail.

## C.credential-compromise.md

# Runbook: Credential compromise / incident response

Use when an API key, OAuth token, connector credential, secret reference, or signing credential is suspected or confirmed compromised.

1. Identify the affected `CredentialProfile`, provider/tenant, scopes, dependent connectors/models and last-known-good time.
2. Immediately set the profile to suspended/blocked in MAIA so no new Run may select it.
3. Revoke/disable the credential at the authoritative provider or tenant control plane; do not rely only on local deletion.
4. Invalidate dependent cached sessions, subscriptions/webhooks and background jobs where the credential could still authorize work.
5. Rotate/reissue the credential with the minimum required scopes and store it through the native vault flow.
6. Audit `Run`, `Event` and `AuditRecord` entries from the last-known-good time through revocation; identify external side effects and unusual scope use.
7. Assess blast radius: data read, data written/sent, external recipients, MCP/tool calls, tenant resources and any downstream secret exposure.
8. Follow organization incident/privacy/security notification procedures where applicable; MAIA records the incident but does not decide legal notification obligations.
9. Re-enable the profile only after validation and policy review.
10. Record root cause, affected versions/scopes, rotation time, follow-up actions and tests that prevent recurrence.

**Fail closed:** if authoritative revocation cannot be confirmed, the profile remains blocked.

## C.credential-lifecycle.md

# Runbook: Credential lifecycle

1. User opens Settings > Credentials.
2. Select provider/profile type and requested scopes.
3. Secret is entered only into the dedicated secure form and submitted once to the privileged core.
4. Core stores secret material in OS/enterprise vault and returns only masked metadata.
5. Validate using the cheapest safe provider operation.
6. Rotation creates a new secret version, validates it, switches references atomically, then revokes the old secret when possible.
7. Deletion is blocked while an active Run requires the credential unless the Run is canceled or remapped.
8. Export never contains secret material or reusable vault identifiers.

## C.graph-onboarding.md

# Runbook: Microsoft Graph onboarding

1. Register/identify the approved Entra application pattern for the tenant.
2. Request the least delegated/application permissions needed for the enabled capabilities.
3. Complete admin consent only where policy requires it.
4. Validate identity, tenant, mailbox and scopes.
5. Start bounded initial synchronization; persist delta links per folder/resource.
6. If webhooks are available, create subscriptions and lifecycle notification handling.
7. Record scopes, tenant, consent mode and expiry/renewal state in connector metadata.
8. Do not enable send/delete capabilities unless explicitly granted and policy-approved.

## C.mcp-onboarding.md

# Runbook: MCP server onboarding

1. Add server URL/transport and authentication metadata.
2. Validate TLS/endpoint policy and server identity.
3. Discover tools/resources and cache catalog with expiry.
4. Map every tool to MAIA risk class and required scopes.
5. Block tools whose schemas are ambiguous, privileged or incompatible with policy.
6. Test a read-only operation.
7. Enable write tools only through explicit policy and ApprovalGate.
8. Audit every external tool invocation and sanitized result metadata.

## C.outlook-classic.md

# Runbook: Outlook Classic / TrustedBridge

1. Detect Classic Outlook availability and current user session.
2. Health-check COM access without sending or modifying mail.
3. Restrict the connector to declared mailbox/folder scope.
4. Normalize messages into MAIA domain objects; raw COM objects never cross the connector boundary.
5. Draft creation may be autonomous under policy; send always passes ApprovalGate by default.
6. On client/COM failure, mark connector degraded and do not silently switch to another mailbox connector for a pending side effect.
7. Migration to Graph preserves domain IDs via external reference mapping where possible.
8. **STA apartment:** all Outlook object-model calls execute on a dedicated COM STA thread initialized with `COINIT_APARTMENTTHREADED`. Worker threads communicate with this executor through a queue; raw COM proxies are not passed across arbitrary worker threads.
9. **Busy/modal Outlook:** register/implement `IMessageFilter` (or equivalent COM interop retry handling). `SERVERCALL_RETRYLATER`, `RPC_E_SERVERCALL_RETRYLATER` and `RPC_E_CALL_REJECTED` are treated as bounded transient busy conditions, not `InternalError`. Use bounded increasing retry delay; the default MAIA busy budget is 10 seconds and is configuration/policy data.
10. If the busy budget is exhausted before the external call is accepted, publish `ConnectorHealth=degraded` and return a retryable Run result. Do not hang the sidecar.
11. **Non-idempotent ambiguity:** if a send/move call may have crossed the side-effect boundary but the response is lost/ambiguous, set `RunState=outcome_unknown` and `ActionState=reconciling`. Never retry automatically until reconciliation proves whether the effect occurred.
12. For retry/reconciliation, prefer stable external IDs and an MAIA action marker where the connector safely supports one. If the outcome cannot be proven, require explicit human resolution rather than risking a duplicate send.

## C.release.md

# Runbook: Release

1. Validate every YAML spec and generated contract.
2. Run unit, connector-contract, E2E, migration and security suites.
3. Run dependency audit and secret scan.
4. Refresh external source snapshot for Microsoft/MCP/Viktor benchmark references.
5. Build installer/service packages.
6. Verify signed artifacts and clean-machine first run.
7. Test upgrade from previous supported schema with backup/rollback.
8. Render current master documentation and inspect layout.
9. Tag release only when target acceptance vector is green.

## C.side-effect-reconciliation.md

# Runbook: Ambiguous side-effect reconciliation

Use when a connector reports timeout/disconnect/unknown outcome after a non-idempotent operation such as mail send, calendar invitation, external post or destructive action.

1. Set the current Run to `outcome_unknown` and Action to `reconciling`; block automatic retry.
2. Record the exact `action_hash`, connector identity, external target, timestamps and any idempotency/action marker.
3. Query the authoritative external system for evidence that the side effect occurred.
4. If execution is proven, mark Run/Action completed and store the external result reference.
5. If non-execution is proven, transition to retryable state and perform a fresh policy/source-version check before retry.
6. If outcome remains ambiguous, keep the action blocked and request human resolution; do not guess.
7. Any material source/payload change during reconciliation invalidates previous approval.

## C.sync-recovery.md

# Runbook: Sync recovery

1. Detect expired subscription, invalid delta cursor or missed notification signal.
2. Pause derived proactive actions that depend on incomplete state.
3. Attempt subscription renewal or safe cursor continuation.
4. If cursor is invalid, run bounded resynchronization for affected folders/resources.
5. Deduplicate by connector/external ID/version.
6. Recompute derived entities (threads, commitments, WorkGraph edges) idempotently.
7. Record recovery event and completeness status.

## C.teams-agent.md

# Runbook: Teams / Microsoft 365 Agent Surface

1. Deploy/register a managed public HTTPS channel endpoint for production Teams/M365 traffic.
2. Treat Dev Tunnel/localhost exposure as development-only.
3. Resolve workspace state authority before accepting work: managed Core/store or local-authoritative Core.
4. For local-authoritative workspaces, establish an authenticated outbound-only relay from local Core/worker to the managed Channel Adapter; Azure Relay Hybrid Connections is the reference pattern, not a protocol requirement.
5. The Channel Adapter validates channel identity and emits a signed/authenticated MAIA envelope. It does not plan actions or bypass Core policy.
6. Envelopes are workspace/actor bound, idempotent, replay-protected and expiring.
7. Approval cards contain the canonical `approval_id`, `action_hash` and `expected_version`; final decision is CAS in authoritative Core.
8. If local Core is offline, queue only bounded/expiring envelopes. Never execute local-authority side effects in the cloud adapter.
9. Do not access/copy the workstation SQLite file from the cloud component.
10. For enterprise managed mode, state is held in the managed transactional store; desktop acts as thin client/capability worker as configured.

# ZAŁĄCZNIK D - Implementation Pack manifest

```text
AGENTS.md
README.md
assets/architecture.dot
assets/architecture.png
assets/migration.dot
assets/migration.png
assets/task_lifecycle.dot
assets/task_lifecycle.png
assets/workgraph.dot
assets/workgraph.png
docs/CHANGELOG-v1.1.md
docs/CHANGELOG-v1.2.md
docs/MAIA_Dokumentacja_v1.1.md
docs/MAIA_Dokumentacja_v1.2.md
docs/MAIA_Dokumentacja_v1.md
docs/adr/ADR-0001.md
docs/adr/ADR-0002.md
docs/adr/ADR-0003.md
docs/adr/ADR-0004.md
docs/adr/ADR-0005.md
docs/adr/ADR-0006.md
docs/adr/ADR-0007.md
docs/adr/ADR-0008.md
docs/adr/ADR-0009.md
docs/adr/ADR-0010.md
docs/adr/ADR-0011.md
docs/adr/ADR-0012.md
docs/adr/ADR-0013.md
docs/adr/ADR-0014.md
docs/adr/ADR-0015.md
docs/adr/ADR-0016.md
docs/adr/ADR-0017.md
docs/adr/ADR-0018.md
docs/adr/ADR-0019.md
docs/adr/ADR-0020.md
docs/adr/ADR-0021.md
docs/adr/ADR-0022.md
docs/adr/ADR-0023.md
docs/adr/ADR-0024.md
docs/adr/ADR-0025.md
docs/adr/ADR-0026.md
docs/adr/ADR-0027.md
docs/adr/ADR-0028.md
docs/adr/ADR-0029.md
docs/adr/ADR-0030.md
docs/runbooks/approval-incident.md
docs/runbooks/credential-compromise.md
docs/runbooks/credential-lifecycle.md
docs/runbooks/graph-onboarding.md
docs/runbooks/mcp-onboarding.md
docs/runbooks/outlook-classic.md
docs/runbooks/release.md
docs/runbooks/side-effect-reconciliation.md
docs/runbooks/sync-recovery.md
docs/runbooks/teams-agent.md
generated/acceptance.md
generated/approval.md
generated/canonical_registry.md
generated/spec_appendix.md
generated/spec_guard_report.txt
generated/states.md
generated/testing.md
locales/en.json
locales/pl.json
prompts/mail_triage.md
spec/acceptance.yaml
spec/approval.yaml
spec/compliance.yaml
spec/connectors.yaml
spec/deployment.yaml
spec/domain.yaml
spec/errors.yaml
spec/ipc.yaml
spec/mail.yaml
spec/mcp.yaml
spec/memory.yaml
spec/models.yaml
spec/modes.yaml
spec/persistence.yaml
spec/product.yaml
spec/scheduler.yaml
spec/security.yaml
spec/sources.snapshot.yaml
spec/states.yaml
spec/sync.yaml
spec/testing.yaml
spec/transport.yaml
spec/ux.yaml
spec/workgraph.yaml
tests/spec/jcs_rfc8785_vectors.json
tools/generate_document_fragments.py
tools/jcs_reference.mjs
tools/spec_guard.py
tools/validate_spec.py
```
