---
title: "MAIA - Multichannel Automation & Intelligent Assistance"
subtitle: "Dokumentacja Techniczna i Architektura Wykonawcza v1.1"
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

![Architektura referencyjna MAIA](MAIA_v1_1_Implementation_Pack/assets/architecture.png){ width=90% }

## 0. Streszczenie wykonawcze

MAIA - **Multichannel Automation & Intelligent Assistance** - nie jest dodatkiem do Outlooka i nie jest chatbotem odpowiadającym na pojedyncze pytania. Rozwinięcie akronimu zostało w v1.1 zmienione z historycznego „Mail Automation” na „Multichannel Automation”, aby nazwa produktu odpowiadała channel-neutralnej architekturze; Mail Intelligence pozostaje jednym z jego strategicznych wyróżników. Jest warstwą operacyjną nad komunikacją i pracą użytkownika: odbiera intencję, buduje plan, dobiera model i connector, kontroluje ryzyko, wykonuje dozwolone działania, zapisuje proweniencję i aktualizuje długotrwały kontekst pracy. Poczta jest jednym z najważniejszych źródeł i kanałów działania, ale pozostaje wymiennym connector-em.

Obecna ścieżka Outlook Classic + TrustedBridge zostaje zachowana jako `OutlookClassicConnector`. Dzięki temu prototyp działający w ograniczonym środowisku firmowym nie jest wyrzucany. Jednocześnie MAIA Core nie zależy od COM. Gdy organizacja przejdzie na nowy Outlook, connector może zostać zastąpiony Microsoft Graph i Outlook Web Add-in bez migracji całej inteligencji, pamięci, WorkGraphu i historii audytowej.

Najważniejszą przewagą MAIA ma być głębokość rozumienia komunikacji: powiązanie ludzi, projektów, maili, spotkań, decyzji, zobowiązań i plików w jeden WorkGraph. MAIA nie tylko streszcza wiadomość; potrafi rozpoznać, że mail zmienia wcześniejsze ustalenie, że odpowiedź jest oczekiwana, że ktoś obiecał dostarczyć element do określonego terminu i że ta obietnica wpływa na następne spotkanie.

**v1.1 Contract Correction:** ta wersja zamyka wykryte w review luki kontraktowe v1.0: dynamiczną klasyfikację ryzyka, pełny `ConnectorHealth` FSM, mapowanie gate -> `ApprovalState`, concurrency Approval, MCP tool-definition pinning, credential incident response, jawny platform boundary oraz governance danych osób trzecich. `tools/validate_spec.py` jest wykonywalnym Spec Guardem, nie deklaracją przyszłej funkcji.

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

![Migracja connectorów bez migracji rdzenia](MAIA_v1_1_Implementation_Pack/assets/migration.png){ width=90% }

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

Architektura składa się z powierzchni interakcji, MAIA Agent Runtime, inteligencji domenowej, routerów, connectorów oraz lokalnej/enterprise warstwy danych. Agent runtime nie powinien wiedzieć, czy wiadomość została pobrana przez COM, Graph czy przyszły connector Gmail; pracuje na znormalizowanym modelu.

### 5.1. Interaction Gateway

Normalizuje komendy z desktopu, Teams, Outlook Add-in i API/MCP do wspólnego `AgentTask`. Zachowuje origin surface i conversation reference, ale nie duplikuje logiki planowania.

### 5.2. Planner / Executor

Planner tłumaczy intencję na `ExecutionPlan` z uporządkowanymi `Action`. Każda akcja ma wymagane capabilities, ryzyko, connector preference, model requirement i warunki sukcesu. Executor wykonuje wyłącznie plan po przejściu polityk.

### 5.3. Result Verifier

Po operacji MAIA nie zakłada sukcesu na podstawie braku wyjątku. Connector zwraca znormalizowany wynik oraz zewnętrzny identyfikator. Dla krytycznych operacji możliwa jest kontrola read-after-write, np. potwierdzenie, że draft istnieje albo że event kalendarza ma oczekiwanych uczestników.

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

Dokładne listy i przejścia są normatywnie utrzymywane wyłącznie w `spec/states.yaml`. Każdy FSM ma jawny mapping przejść; `validated_at_runtime` nie jest legalnym substytutem specyfikacji. Błędne przejście zwraca `InvalidStateTransition`, zapisuje event i nie jest naprawiane po cichu.

**AgentTaskState**

- **Stany:** draft, preflight, awaiting_approval, queued, running, paused, partially_completed, completed, canceled, failed

- **Przejścia:** {"awaiting_approval": ["queued", "canceled", "failed"], "draft": ["preflight", "canceled"], "partially_completed": ["queued", "completed", "failed", "canceled"], "paused": ["queued", "canceled"], "preflight": ["awaiting_approval", "queued", "failed", "canceled"], "queued": ["running", "canceled", "failed"], "running": ["paused", "partially_completed", "completed", "failed", "canceled"]}

- **Semantyka:** domain state machine

**ActionState**

- **Stany:** planned, gated, awaiting_approval, queued, running, retryable_error, completed, skipped, failed, canceled

- **Przejścia:** {"awaiting_approval": ["queued", "canceled", "failed"], "gated": ["awaiting_approval", "queued", "skipped", "failed"], "planned": ["gated", "canceled"], "queued": ["running", "canceled"], "retryable_error": ["queued", "failed", "canceled"], "running": ["retryable_error", "completed", "failed", "canceled"]}

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

![Cykl wykonania zadania](MAIA_v1_1_Implementation_Pack/assets/task_lifecycle.png){ width=90% }

## 9. Agent Orchestrator

### 9.1. Preflight

1. Ustal origin, workspace i tożsamość użytkownika.
2. Określ klasę prywatności danych i wymagane capabilities.
3. Rozpoznaj, czy zadanie jest tylko odczytem/analizą, czy zawiera side effects.
4. Zbuduj plan i przypisz risk class każdej akcji.
5. Sprawdź connector health, scopes i credential profiles.
6. Dobierz model zgodny z polityką i klasyfikacją danych.
7. Wylicz wymagane zgody, ewentualny koszt i potencjalny egress.
8. Pokaż użytkownikowi plan, jeśli wymaga tego polityka lub ryzyko.

### 9.2. Wykonanie

Akcje niezależne mogą wykonywać się równolegle, ale operacje zależne od wcześniejszego wyniku są serializowane. Każda próba tworzy nowy `Run`. Retry nie zmienia logicznego `Action`, a fallback connector/model jest jawny.

### 9.3. Human intervention

Użytkownik może pauzować, anulować, modyfikować plan, zatwierdzać pojedyncze działania lub ustawiać reguły. Każda materialna zmiana planu po zatwierdzeniu wymaga ponownego preflightu i - jeśli zmienia payload side effect - nowej zgody.

## 10. Policy Router, Privacy Gate i Approval Gate

Router zaczyna od reguł twardych, a dopiero potem ocenia preferencję, jakość, koszt i latencję. Kluczowa korekta v1.1: **risk class nie zawsze jest statyczną właściwością komendy**. `mail.send` i operacje kalendarzowe z uczestnikami klasyfikują ryzyko z *znormalizowanego payloadu* przed uruchomieniem ApprovalGate.

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

`policy` nie oznacza automatycznie ani `pending`, ani `not_required`: polityka musi jawnie zwrócić możliwość autonomicznego wykonania; w przeciwnym razie powstaje `pending`. `confirm` i `elevated_confirm` zawsze tworzą `pending`.

### 10.2. Dynamic risk classification

- `mail_recipient_boundary`: wszyscy odbiorcy wewnętrzni -> `send_internal`; dowolny odbiorca poza zatwierdzoną granicą organizacji -> `send_external`.
- `calendar_participant_boundary`: prywatny event bez uczestników -> `write_internal`; event z uczestnikami wewnętrznymi -> `send_internal`; dowolny zewnętrzny uczestnik -> `send_external`.
- Resolver **musi** zakończyć się legalną wartością `RiskClass` zanim zostanie wybrana bramka. Nie istnieją hybrydowe enumy typu `send_external_or_internal`.

### 10.3. Recipient Safety

Poczta wymaga dodatkowych zabezpieczeń domenowych: różnica odbiorców między oryginalnym wątkiem a propozycją, zewnętrzne domeny, reply-all, BCC, duża liczba odbiorców, nowe załączniki i zmiana klasyfikacji. Approval Card pokazuje te różnice wprost.

### 10.4. Approval concurrency

Approval może być widoczny jednocześnie na desktopie, w Teams i na innym autoryzowanym urządzeniu. Decyzja jest `compare-and-swap` względem `approval_id + action_hash + expected_version + state=pending`. **Pierwsza terminalna decyzja wygrywa**. Każda druga/stara decyzja zwraca `ExternalConflict`. Materialna edycja planu unieważnia starą zgodę i tworzy nową wersję; tuż przed side effectem Executor ponownie sprawdza `approved` + zgodność `action_hash`.

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

To ścieżka produkcyjnie użyteczna w środowisku, w którym użytkownik ma działającego Outlook Classic i nie ma zgody na Graph/API. Connector powinien działać w osobnym procesie/sidecarze o minimalnych uprawnieniach, normalizować obiekty COM i zwracać tylko dane domenowe. Core nie importuje bibliotek Outlook Interop i nie zna szczegółów EntryID/MAPI poza mapowaniem zewnętrznych referencji.

Kierunek rozwoju obecnego TrustedBridge: stabilny kontrakt request/response, jawne limity czasu, health check, cancellation tam gdzie możliwe, deduplikacja, bezpieczne okno zgody przed wysłaniem oraz diagnostyka bez treści sekretnej. PowerShell może pozostać warstwą bootstrapową, ale kontrakt powinien umożliwić późniejsze przepisanie sidecara bez zmiany Core.

## 14. Microsoft Graph Connector

Graph jest docelowym connector-em M365 dla maila, kalendarza i części danych plikowych. Synchronizacja skrzynki powinna wykorzystywać delta query per folder, a tam gdzie deployment na to pozwala - change notifications/webhooks. Microsoft opisuje oba mechanizmy jako sposób utrzymania lokalnego stanu bez pełnego pobierania skrzynki [MS-04, MS-05].

Connector przechowuje deltaLink/cursor, stan subskrypcji, tenant i scopes. Lifecycle notification lub utrata kursora uruchamia runbook odzyskiwania. Delivery semantycznie traktujemy jako at-least-once: zdarzenia są deduplikowane, a derivation WorkGraphu jest idempotentna.

### 14.1. Uprawnienia

MAIA prosi tylko o capabilities rzeczywiście włączone. Profil tylko do analizy poczty nie wymaga send/delete. W środowisku enterprise admin może centralnie zablokować określone capability niezależnie od dostępnego tokenu. Connector widzi token jako sekret; Orchestrator widzi jedynie wynik capability check.

## 15. Teams i Microsoft 365 Agent Surface

Teams jest interfejsem, a nie odrębnym agentem. Microsoft 365 Agents SDK jest zaprojektowany jako warstwa komunikacyjna dla wielu kanałów i nie narzuca modelu AI, co odpowiada architekturze MAIA [MS-01]. Microsoft dokumentuje także możliwość rozszerzania agentów poza Teams do Outlook/M365 [MS-02].

W direct chat użytkownik może zlecić zadanie, zapytać o stan i zatwierdzić akcję. W kanałach zespołowych agent powinien mieć osobną politykę zasięgu: samo dodanie do kanału nie oznacza prawa do czytania historycznej treści ani wykonywania działań w imieniu każdej osoby.

### 15.1. Adaptive Cards

Najważniejsze karty: Task Plan, Approval, Meeting Brief, Commitment Alert, Awaited Reply, Result Summary. Karta musi zawierać task ID i wersję payloadu, aby decyzja z Teams nie mogła zatwierdzić później zmodyfikowanej akcji.

## 16. Mail Intelligence

Mail Intelligence jest specjalistycznym modułem, który ma sprawić, że MAIA wyprzedzi ogólnych agentów. Każda wiadomość jest analizowana nie tylko jako tekst, lecz jako zdarzenie w relacji i projekcie. Pipeline obejmuje threading, entity linking, projekt, intencję, decyzje, commitments, urgency i potrzebę odpowiedzi.

**summary**

- **Rola:** Standardowy wynik analizy wiadomości/wątku



**why_it_matters**

- **Rola:** Standardowy wynik analizy wiadomości/wątku



**suggested_action**

- **Rola:** Standardowy wynik analizy wiadomości/wątku



**draft_reply**

- **Rola:** Standardowy wynik analizy wiadomości/wątku



**commitments**

- **Rola:** Standardowy wynik analizy wiadomości/wątku



**decisions**

- **Rola:** Standardowy wynik analizy wiadomości/wątku



**project_links**

- **Rola:** Standardowy wynik analizy wiadomości/wątku



**risk_flags**

- **Rola:** Standardowy wynik analizy wiadomości/wątku



### 16.1. Why it matters

Streszczenie odpowiada na „co napisano”. Pole `why_it_matters` odpowiada na „dlaczego powinienem się tym zająć”. Może wskazać oczekiwaną odpowiedź, konflikt z wcześniejszą decyzją, zmianę terminu, nowego zewnętrznego odbiorcę, zaległe zobowiązanie lub zbliżające się spotkanie.

### 16.2. Draft generation

Draft jest zawsze osobnym artefaktem. Model otrzymuje zwięzły ContextPack: cel odpowiedzi, fakty źródłowe, ostatnie ustalenia, relationship style i ograniczenia. Nie powinien otrzymywać całej skrzynki. Każde stwierdzenie operacyjne mające znaczenie może zostać powiązane z source_ref.

## 17. Calendar & Meeting Intelligence

Spotkanie jest węzłem WorkGraphu. Przed spotkaniem MAIA tworzy brief: uczestnicy, relacja, ostatnie wątki, otwarte commitments, decyzje, ryzyka i dokumenty. Po spotkaniu transkrypcja/notatki są traktowane jako niezaufane źródło danych, z którego można wydobyć decyzje i candidate commitments.

Tworzenie, przenoszenie i anulowanie spotkania jest side effectem klasyfikowanym dynamicznie: prywatny hold może być `write_internal`, zaproszenie tylko uczestników wewnętrznych `send_internal`, a obecność dowolnego zewnętrznego uczestnika `send_external`. Materialna zmiana istniejącego spotkania może dodatkowo podlegać regułom destructive/change policy.

**Ograniczenie `local_legacy`:** canonical profil `outlook_classic_com` udostępnia tylko `calendar.read`, nie `calendar.write`. W tym trybie MAIA może budować briefingi i analizować kalendarz, ale nie może kanonicznie tworzyć/przenosić/anulować spotkań. CapabilityGate odrzuca takie Action jako `CapabilityMissing`. Write calendar staje się dostępny po użyciu connectora deklarującego `calendar.write` (docelowo Microsoft Graph).

## 18. WorkGraph

WorkGraph jest najważniejszą trwałą strukturą wiedzy MAIA. Nie jest grafem „wszystkiego o człowieku”. Jest grafem pracy: kto, nad czym, z kim, w jakim wątku, jakie decyzje i zobowiązania. Każda krawędź ma źródło i confidence; inference jest oznaczone inaczej niż relacja jawna.

![Model WorkGraph](MAIA_v1_1_Implementation_Pack/assets/workgraph.png){ width=80% }

Przykład: mail od dostawcy może połączyć `Person -> Organization`, `Thread -> Project`, wygenerować `Commitment`, a załącznik `Artifact`. Gdy później użytkownik pyta „co zostało z Gietzem?”, MAIA odtwarza stan projektu z grafu zamiast przeszukiwać chaotyczną historię czatu.

## 19. Commitment Engine

Commitment jest pierwszorzędną encją. System rozróżnia kandydat, propozycję i zobowiązanie potwierdzone. Zdanie „postaram się wysłać” nie jest równoznaczne z „wyślę do piątku”. Ekstraktor zapisuje owner, beneficiary, treść, termin, źródło i confidence. Reguły mogą automatycznie potwierdzać tylko bardzo jednoznaczne zobowiązania lub zawsze wymagać review - zależnie od workspace policy.

Proaktywność MAIA opiera się na commitments: przypomnienie przed terminem, wykrycie overdue, powiązanie odpowiedzi potwierdzającej wykonanie, przygotowanie follow-upu i uwzględnienie w briefingach.

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

MAIA jest zarówno klientem MCP, jak i konserwatywnym serwerem MCP. Klient może odkrywać zatwierdzone zewnętrzne tools/resources, lecz każdy tool przechodzi normalne Policy/Privacy/Approval gates. Serwer MAIA domyślnie wystawia tylko wąskie, jawne scope'y; `mail.send`, `mail.delete`, `credentials.manage` i `policy.modify` nie są domyślnymi scope'ami.

### 24.1. Tool-definition pinning / rug-pull protection

Samo `tools/list` + cache/revalidation nie wystarcza. Zdalny serwer może zmienić opis lub schema narzędzia po wcześniejszym review. MAIA tworzy canonical SHA-256 fingerprint obejmujący **server identity + tool name/title + description + inputSchema + outputSchema + annotations**. Trust decision i Approval odnoszą się do tego fingerprintu. Jeśli definicja zmieni się przed execution, poprzednia zgoda/allowlist decision jest nieważna; polityka jest liczona ponownie, a stale execution zwraca `ExternalConflict`/`McpToolBlocked` zgodnie z kontekstem.

Tool annotations i tool output są niezaufane. MCP nie może nadawać sobie scope'ów, omijać ApprovalGate ani eskalować z read do write na podstawie własnego opisu.

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

Baseline to SQLite + FTS5 na desktopie, z repository traits/interfaces pozwalającymi później zastosować enterprise store. Baza zawiera domenę, proweniencję, cursory synchronizacji i audit metadata, ale nie sekrety. Indeksy wyszukiwawcze również nie mogą zawierać credentiali ani niepotrzebnych danych diagnostycznych.

Migracje są wersjonowane, poprzedzone backupem i testowane na fixture starej wersji. Błąd migracji blokuje zapis i oferuje recovery; aplikacja nigdy nie „naprawia” problemu przez skasowanie bazy.

## 29. Synchronizacja i zdarzenia

COM, Graph i Teams mają różne mechanizmy zdarzeń, dlatego Core przyjmuje wspólne semantyczne eventy. Delivery traktujemy jako at-least-once. Każdy event ma idempotency key. Graph delta link i webhook subscription są stanem connectora, nie modelu domenowego.

W nowoczesnym M365 Graph delta query pozwala pobierać przyrostowe zmiany wiadomości, a change notifications dostarczają niskolatencyjne sygnały; lifecycle notifications wymagają obsługi odnowień i braków [MS-04, MS-05].

## 30. Notifications i Approval UX

Approval może zostać przedstawiony na dowolnej autoryzowanej powierzchni, ale ma **jedną kanoniczną tożsamość i wersję**. Desktop, Teams i Outlook UI pokazują ten sam `approval_id`, `action_hash`, version, risk class, target/recipients, attachment diff i side-effect summary. UI nie może zatwierdzać „intencji” bez związania z dokładnym payloadem.

### 30.1. Decyzje równoczesne

Każdy przycisk Approve/Reject wysyła `expected_version`. Core wykonuje atomic compare-and-swap tylko gdy Approval nadal jest `pending` i hash odpowiada bieżącej Action. Pierwsza terminalna decyzja wygrywa. Druga powierzchnia dostaje `ExternalConflict` i odświeża kartę zamiast nadpisywać decyzję. Edycja planu/payloadu revoke/expire'uje starą zgodę i tworzy nową wersję.

### 30.2. Notyfikacje

Proaktywne alerty (meeting brief, awaited reply, overdue commitment, credential incident) respektują workspace notification policy. Sam fakt dostarczenia notyfikacji nie daje nowego uprawnienia do wykonania działania.

## 31. Error Taxonomy, retry i idempotency

Każdy błąd connectora/modelu jest mapowany na kanoniczny AppErrorCode. Retryability jest jawna. Timeout, 429 i część błędów 5xx mogą być retryable; recipient risk, policy block i credential rejected nie powinny być naprawiane automatycznym retry. Akcje side-effect mają idempotency key, aby awaria sieci nie doprowadziła do podwójnego wysłania.

**Kanoniczne kody**

- **Lista:** InvalidStateTransition, ApprovalRequired, ApprovalExpired, PolicyBlocked, PrivacyBlocked, CapabilityMissing, ConnectorUnavailable, CredentialMissing, CredentialRejected, RateLimited, Timeout, Canceled, ExternalConflict, RecipientRisk, AttachmentRisk, PromptInjectionSuspected, McpToolBlocked, SyncCursorInvalid, SubscriptionExpired, DuplicateAction, UnknownCost, PersistenceError, MigrationError, ValidationError, UnsupportedClient, InternalError



## 32. Security Threat Model

Model zagrożeń obejmuje nie tylko prompt injection i secret exfiltration, ale również wyścigi approval, zmienną definicję MCP tool oraz kompromitację credentiali. Poniższy rejestr jest renderowany z `spec/security.yaml`.

**prompt_injection_mail_or_attachment**

- **Kontrole:** untrusted_content_boundary, instruction_source_labels, no_tool_call_from_untrusted_text_without_planner_validation

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

### 32.1. Invariants

- human_authority
- least_privilege
- all_side_effects_are_auditable
- secrets_are_never_plaintext_persistent
- content_is_data_not_instruction
- enterprise_policy_can_be_stricter_than_user_policy

## 33. Klasyfikacja danych, prywatność i dane osób trzecich

WorkGraph, `Person`, `RelationshipProfile`, uczestnicy maili/spotkań i `MemoryClaim` mogą zawierać dane osób, które nie są użytkownikami MAIA. To nie jest „tylko prywatna pamięć użytkownika” i v1.1 traktuje ten fakt jako jawny problem governance.

MAIA implementuje techniczne zasady: purpose limitation, data minimization, accuracy/source provenance, storage limitation, ograniczenie wnioskowania o cechach wrażliwych oraz subject-linked operacje locate/export/rectify/restrict/erase-or-anonymize. Jednocześnie produkt **nie wybiera sam podstawy prawnej**, nie rozstrzyga roli controller/processor i nie obiecuje bezwarunkowego usunięcia rekordu, gdy prawo/polityka retencji lub obowiązek audytowy wymaga zachowania określonych danych. Te kwestie są profilem wdrożeniowym i gate'em DPO/privacy/legal dla enterprise.

### 33.1. Third-party subject workflow

- locate_subject_linked_records
- export_subject_linked_records
- rectify_or_supersede_inaccurate_records
- restrict_or_suppress_processing_by_policy
- erase_or_anonymize_where_policy_and_law_permit
- propagate_deletion_to_indexes_and_derived_memory_subject_to_audit/legal-retention rules

Indeksy, embeddingi, derived memory i Relationship Intelligence muszą respektować korektę/usunięcie/suppression źródłowego podmiotu zgodnie z configured policy; audit może zachować minimalny, odseparowany ślad operacyjny, jeśli polityka prawna tego wymaga.

### 33.2. Governance pozostające decyzją wdrożeniową

- **controller_processor_roles:** deployment_decision
- **legal_basis:** deployment_decision
- **retention_schedule:** workspace_or_tenant_policy
- **data_subject_request_owner:** deployment_decision
- **enterprise_release_gate:** DPO/privacy/legal review required for the actual deployment context

Oficjalne źródła UE/EDPB wskazują m.in. prawa dostępu, sprostowania, usunięcia i ograniczenia przetwarzania; prawo do usunięcia nie jest absolutne. Master traktuje te źródła jako guidance/compliance snapshot, a nie poradę prawną dla konkretnego wdrożenia. [EU-01, EU-02]

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

Windows jest **platformą referencyjną Alpha**, nie kontraktem całego produktu. Powód jest pragmatyczny: bootstrap `outlook_classic_com` wymaga Windows. Domain, Orchestrator, Policy/Approval, WorkGraph, Model Router i connector contracts mają być OS-neutralne, a bezpośrednie importy COM/Win32 poza adapterem są defektem architektonicznym.

Graph, Teams Agent Surface, MCP oraz zarządzany service nie mają wymogu Windows wynikającego z kontraktu MAIA. To nie oznacza automatycznej obietnicy desktopowego supportu macOS/Linux w MVP - support matrix pozostaje decyzją roadmapową - ale architektura nie może go zablokować.

Przed `mvp_universal` core/spec/connector-contract suites mają przechodzić na co najmniej jednym non-Windows CI runnerze. Platform-specific testy Classic Outlook pozostają Windows-only.

Update jest podpisany, poprzedzony schema preflight i backupem. Microsoft Teams/M365 deployment zależy od rejestracji aplikacji, consentu i tenant policy; jest deployment profile, a nie warunkiem uruchomienia `local_legacy`.

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

- **Zakres:** schema_valid, unique_ids, state_transitions, acceptance_refs, forbidden_secret_fields, generated_drift, risk_enum_integrity, dynamic_risk_classifier_refs, approval_gate_state_mapping, all_fsm_transitions_explicit

**core**

- **Zakres:** planner, approval_gate, privacy_gate, model_router, connector_router, workgraph, commitments, memory, scheduler, idempotency, approval_concurrency, approval_action_hash_recheck

**connector_contract**

- **Zakres:** capability_discovery, health, cancellation, timeouts, error_mapping, provenance, auth_failure, rate_limit

**e2e**

- **Zakres:** classic_read_to_draft, teams_request_to_approval, graph_delta_resync, meeting_prebrief, overdue_commitment, api_key_rotation, mcp_read_tool

**security**

- **Zakres:** prompt_injection_mail, recipient_swap, reply_all, ssrf, webhook_replay, malicious_mcp, secret_grep, archive_bomb, policy_bypass, mcp_tool_definition_rug_pull, credential_compromise_response, stale_approval_decision

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

### 40.2. Spec Guard jako blocker M0

`python tools/validate_spec.py` musi być zielone przed wejściem w implementację pionowego slice. v1.1 pack: **PASS - 23 YAML files parsed and contract checks passed**.

## 41. Acceptance Criteria

Kryteria poniżej są widokiem `spec/acceptance.yaml`, a nie drugim źródłem prawdy.

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

**M0 Canonical Spec / Repo**

- **Zakres:** spec, ADR, generator, CI skeleton

- **Exit condition:** Spec Guard green; pack is authoritative



**M1 Core Task Runtime**

- **Zakres:** Task/Plan/Action/Run, events, audit

- **Exit condition:** One local no-op/read task end-to-end



**M2 Outlook Classic Adapter**

- **Zakres:** Wrap current TrustedBridge/COM behind connector contract

- **Exit condition:** Read/search/draft flow without core COM dependency



**M3 Mail Intelligence**

- **Zakres:** threading, summary, why-it-matters, draft context

- **Exit condition:** Priority inbox and safe draft end-to-end



**M4 WorkGraph + Commitments**

- **Zakres:** people/projects/decisions/commitments

- **Exit condition:** Source-linked project context survives restart



**M5 Approval / Recipient Safety**

- **Zakres:** risk classes, payload hash, cards

- **Exit condition:** No send without correct gate; reply-all tests



**M6 Model Router / Credential Vault**

- **Zakres:** multi-provider profiles, rotation

- **Exit condition:** Model/key change without code change



**M7 Teams Surface**

- **Zakres:** M365 Agents SDK/Teams direct chat + Adaptive Cards

- **Exit condition:** Request->approval->result through Teams



**M8 Microsoft Graph**

- **Zakres:** mail/calendar sync, delta, notifications

- **Exit condition:** Graph can replace Classic without history loss



**M9 Scheduler / Proactivity**

- **Zakres:** briefings, watches, follow-ups

- **Exit condition:** Daily brief + awaited reply + commitment alert



**M10 MCP Gateway**

- **Zakres:** client + conservative server

- **Exit condition:** Approved external tool + read-only MAIA tools



**M11 Enterprise Governance**

- **Zakres:** tenant policies, deployment profiles, managed auth

- **Exit condition:** Stricter admin policy enforced and audited



**M12 Release hardening**

- **Zakres:** a11y, signed updates, migration, perf, threat regression

- **Exit condition:** v1.0 acceptance green



## 43. AGENTS.md - zasady dla modeli kodujących

```markdown
# MAIA v1 - Agent Instructions

## Read this first
You are working on MAIA v1.1.0. The product is a universal executive agent, not an Outlook macro.

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
- Regenerate normative document fragments with `python tools/generate_document_fragments.py`; CI uses `--check`. Do not hand-sync duplicated enum/state/acceptance lists.

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

**Runbook:** Teams agent deployment

1. Provision MAIA channel app/agent registration using approved tenant tooling.
2. Map Teams identity to MAIA workspace identity.
3. Enable direct chat first; channel access requires explicit installation/invite policy.
4. Incoming activity becomes an AgentTask origin record; no separate Teams-only brain.
5. Use Adaptive Cards for plans, approvals and result summaries.
6. Proactive messages require an existing authorized conversation reference and respect notification policy.
7. Channel data is never treated as trusted instruction merely because it came from Teams.

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

- **Tytuł:** Teams is a first-class MAIA surface

- **Decyzja:** Expose the same task runtime through Teams/M365 Agents SDK channel abstractions.

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

- **Tytuł:** MCP tool-definition pinning

- **Decyzja:** Bind tool trust and any approval to a canonical SHA-256 fingerprint of server identity plus semantically relevant tool definition fields. Definition change invalidates prior trust/approval and reruns policy.

**ADR-0024**

- **Tytuł:** Approval concurrency is optimistic and versioned

- **Decyzja:** Use compare-and-swap on approval id, action hash, expected version and pending state. First terminal decision wins; stale/second attempts return ExternalConflict.

**ADR-0025**

- **Tytuł:** Third-party personal-data governance is explicit

- **Decyzja:** Provide technical controls for subject-linked locate/export/rectify/restrict/erase-or-anonymize workflows while leaving controller/processor role, legal basis and exact retention to deployment governance and DPO/legal review.

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

Microsoft 365 Agents SDK deklaruje channel abstraction i brak lock-in do konkretnej usługi AI [MS-01]. Teams documentation wskazuje Teams SDK dla doświadczeń Teams oraz Microsoft 365 Agents SDK do rozszerzenia poza Teams do Outlook/M365 [MS-02]. Z punktu widzenia MAIA oznacza to, że Interaction Gateway powinien korzystać z tych warstw jako transportu, podczas gdy orkiestracja pozostaje we własnym Core.

Nowy Outlook nie obsługuje COM/VSTO; web add-ins są wspieraną ścieżką rozszerzeń [MS-03]. W konsekwencji nie projektujemy następnych modułów Core wokół `Outlook.Application`. Nowe funkcje pocztowe najpierw powstają jako operacje domenowe i capabilities connectora; dopiero potem mają implementację Classic i/lub Graph.

Graph message delta i change notifications stanowią docelową podstawę synchronizacji skrzynki, ale nie są jedynym źródłem prawdy: MAIA przechowuje własny znormalizowany stan, cursory i eventy, a connector może zostać wymieniony [MS-04, MS-05].

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

Źródła są snapshotem informacyjnym. API, SDK, warunki, security guidance i przepisy/guidance mogą się zmieniać; przed releasem lub wdrożeniem enterprise należy odświeżyć właściwy snapshot i review.

## 49. Otwarte decyzje przed rozpoczęciem pełnego kodowania

- Wybrać docelowy język/service host dla Teams/M365 channel service: .NET/C# vs TypeScript/Node, przy zachowaniu wspólnego spec.
- Zdecydować, czy desktop Core pozostaje Rust/Tauri czy część logiki zostanie współdzielona przez osobny local service.
- Ustalić pierwszy realny deployment Graph i dostępne delegated/admin scopes; nie projektować feature'ów tak, jakby wymagane zgody już istniały.
- **Support matrix:** Windows pozostaje Alpha reference platform; przed universal MVP ustalić, czy i kiedy wspieramy desktop macOS/Linux, przy niezmiennym wymogu OS-neutral Core.
- Ustalić retention, zakres mailbox sync i klasyfikację danych dla pierwszego pilota.
- **RODO/GDPR governance:** dla konkretnego wdrożenia ustalić controller/processor role, legal basis, DSR owner/procedure, retention/exceptions i wymagany DPO/privacy/legal sign-off.
- Zdefiniować minimalny WorkGraph na danych pilota, aby nie przeprojektować ontologii przed testami.
- Ustalić, czy enterprise Approval może wymagać reauthentication/second approver dla wybranych klas `elevated_confirm`.

Te punkty są świadomie pozostawione jako decyzje wdrożeniowe/roadmapowe. Nie mogą jednak naruszyć kontraktu: channel-neutral, OS-neutral Core; jawne capabilities; audit; least privilege; source-linked memory; dynamic risk resolution przed side effectem.

# ZAŁĄCZNIK A - Canonical spec/*.yaml

Poniższe pliki są włączone do dokumentu jako generowany widok. Edytowalnym źródłem prawdy pozostają pliki `spec/*.yaml` w Implementation Pack.

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
  enterprise_user:
    core: desktop_or_managed_service
    mail: graph
    channels: teams/outlook
    identity: entra
    policies: tenant_managed
  hybrid_enterprise:
    core: desktop+agent_service
    sensitive_processing: local_or_tenant
    external_models: policy_controlled
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
  calendar.create:
    cancellable: false
    risk_resolution:
      type: dynamic
      classifier: calendar_participant_boundary
      output_enum: RiskClass
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
  fingerprint_algorithm: sha256(canonical_json)
  on_catalog_change: invalidate prior review/approval/allowlist decision for the changed tool definition and rerun
    policy evaluation
  execution_check: Tool fingerprint at execution MUST equal the fingerprint bound to the approved/reviewed action.
  stale_decision_error: ExternalConflict
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
```

## A.product.yaml

```yaml
schema_version: 1
product:
  id: maia
  name: MAIA
  expanded_name: Multichannel Automation & Intelligent Assistance
  version: 1.1.0
  edition: Universal Executive Agent
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
invariants:
- human_authority
- least_privilege
- all_side_effects_are_auditable
- secrets_are_never_plaintext_persistent
- content_is_data_not_instruction
- enterprise_policy_can_be_stricter_than_user_policy
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
      - paused
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
  ActionState:
    states:
    - planned
    - gated
    - awaiting_approval
    - queued
    - running
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
      - retryable_error
      - completed
      - failed
      - canceled
      retryable_error:
      - queued
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
invalid_transition: return InvalidStateTransition and write audit event; never repair silently
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
  connector_contract:
  - capability_discovery
  - health
  - cancellation
  - timeouts
  - error_mapping
  - provenance
  - auth_failure
  - rate_limit
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

# ADR-0004: Teams is a first-class MAIA surface

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
Users should invoke MAIA where work happens.

## Decision
Expose the same task runtime through Teams/M365 Agents SDK channel abstractions.

## Alternatives considered
- Embed the behavior directly in the current client-specific implementation.
- Allow each feature to make its own local decision.

## Why alternatives were rejected
They create duplicated policy, lock-in or behavior that cannot be audited consistently across surfaces.

## Consequences
Adds enterprise deployment complexity but avoids a separate agent brain.

## Security / privacy impact
The implementation must continue to satisfy `spec/security.yaml`, `spec/approval.yaml`, and the workspace policy profile.

## Migration impact
Existing prototype behavior is preserved only when compatible with this ADR. Incompatible implementation details migrate behind the new contract.

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

# ADR-0023: MCP tool-definition pinning

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
MCP tool catalogs are remotely supplied and can change after initial review, creating a rug-pull/tool-poisoning risk.

## Decision
Bind tool trust and any approval to a canonical SHA-256 fingerprint of server identity plus semantically relevant tool definition fields. Definition change invalidates prior trust/approval and reruns policy.

## Alternatives considered
- Keep v1.0 behavior implicit.
- Let each surface/connector decide independently.

## Why alternatives were rejected
They reintroduce ambiguity, platform lock-in, drift or unauditable security behavior.

## Consequences
Adds a deterministic security boundary at the cost of re-review when legitimate tool definitions change.

## Security / privacy impact
Implementation must satisfy `spec/security.yaml`, `spec/approval.yaml` and, where applicable, `spec/compliance.yaml`.

## Migration impact
This ADR is part of the v1.1 contract correction. Existing v1.0 implementations must migrate rather than preserve contradictory behavior.

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

# ADR-0025: Third-party personal-data governance is explicit

**Status:** Accepted  
**Date:** 2026-09-11  
**Decision owner:** MAIA architecture

## Context
WorkGraph and Relationship Intelligence store data about correspondents and meeting participants who are not MAIA users.

## Decision
Provide technical controls for subject-linked locate/export/rectify/restrict/erase-or-anonymize workflows while leaving controller/processor role, legal basis and exact retention to deployment governance and DPO/legal review.

## Alternatives considered
- Keep v1.0 behavior implicit.
- Let each surface/connector decide independently.

## Why alternatives were rejected
They reintroduce ambiguity, platform lock-in, drift or unauditable security behavior.

## Consequences
Adds privacy engineering requirements without hardcoding a legal conclusion that depends on deployment context.

## Security / privacy impact
Implementation must satisfy `spec/security.yaml`, `spec/approval.yaml` and, where applicable, `spec/compliance.yaml`.

## Migration impact
This ADR is part of the v1.1 contract correction. Existing v1.0 implementations must migrate rather than preserve contradictory behavior.

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

# Runbook: Teams agent deployment

1. Provision MAIA channel app/agent registration using approved tenant tooling.
2. Map Teams identity to MAIA workspace identity.
3. Enable direct chat first; channel access requires explicit installation/invite policy.
4. Incoming activity becomes an AgentTask origin record; no separate Teams-only brain.
5. Use Adaptive Cards for plans, approvals and result summaries.
6. Proactive messages require an existing authorized conversation reference and respect notification policy.
7. Channel data is never treated as trusted instruction merely because it came from Teams.

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
docs/MAIA_Dokumentacja_v1.1.md
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
docs/runbooks/approval-incident.md
docs/runbooks/credential-compromise.md
docs/runbooks/credential-lifecycle.md
docs/runbooks/graph-onboarding.md
docs/runbooks/mcp-onboarding.md
docs/runbooks/outlook-classic.md
docs/runbooks/release.md
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
spec/ux.yaml
spec/workgraph.yaml
tools/generate_document_fragments.py
tools/spec_guard.py
tools/validate_spec.py
```
