#!/usr/bin/env python3
"""Generate the non-canonical MAIA 1.9 technical documentation DOCX.

The canonical sources remain spec/*.yaml, ADRs and docs/project. This small
deterministic publisher deliberately writes an external artifact; it never
changes product contracts or handbook source files.
"""

from __future__ import annotations

import argparse
import datetime as dt
import re
from pathlib import Path

from docx import Document
from docx.enum.text import WD_ALIGN_PARAGRAPH
from docx.oxml import OxmlElement
from docx.oxml.ns import qn
from docx.shared import Inches, Pt, RGBColor


ROOT = Path(__file__).resolve().parents[1]
DOCUMENTATION_ROOT = ROOT / ".local" / "exports" / "documentation"
OVERVIEW_OUTPUT = DOCUMENTATION_ROOT / "MAIA_Technical_Overview_v1.9.docx"
FULL_OUTPUT = DOCUMENTATION_ROOT / "MAIA_Dokumentacja_Techniczna_v1.9.docx"


def product_version() -> str:
    text = (ROOT / "spec" / "product.yaml").read_text(encoding="utf-8")
    match = re.search(r"(?ms)^product:\s*\n.*?^\s+version:\s*([^\s#]+)", text)
    if not match:
        raise SystemExit("product.version missing from spec/product.yaml")
    return match.group(1)


def set_cell_shading(cell, color: str) -> None:
    tc_pr = cell._tc.get_or_add_tcPr()
    shade = OxmlElement("w:shd")
    shade.set(qn("w:fill"), color)
    tc_pr.append(shade)


def remove_paragraph_borders(paragraph) -> None:
    """Avoid Word's themed rule on the built-in Title style."""
    p_pr = paragraph._p.get_or_add_pPr()
    border = p_pr.find(qn("w:pBdr"))
    if border is not None:
        p_pr.remove(border)


def remove_style_borders(style) -> None:
    """Remove theme borders inherited from a built-in Word paragraph style."""
    p_pr = style.element.get_or_add_pPr()
    border = p_pr.find(qn("w:pBdr"))
    if border is not None:
        p_pr.remove(border)


def set_cell_border(cell) -> None:
    tc_pr = cell._tc.get_or_add_tcPr()
    borders = tc_pr.first_child_found_in("w:tcBorders")
    if borders is None:
        borders = OxmlElement("w:tcBorders")
        tc_pr.append(borders)
    for side in ("top", "left", "bottom", "right", "insideH", "insideV"):
        edge = OxmlElement(f"w:{side}")
        edge.set(qn("w:val"), "single")
        edge.set(qn("w:sz"), "4")
        edge.set(qn("w:color"), "D9D9D9")
        borders.append(edge)


def keep_row_together(row) -> None:
    tr_pr = row._tr.get_or_add_trPr()
    cant_split = OxmlElement("w:cantSplit")
    tr_pr.append(cant_split)


def repeat_header_row(row) -> None:
    tr_pr = row._tr.get_or_add_trPr()
    repeat = OxmlElement("w:tblHeader")
    repeat.set(qn("w:val"), "true")
    tr_pr.append(repeat)


def add_table(doc: Document, headers: list[str], rows: list[list[str]]) -> None:
    table = doc.add_table(rows=1, cols=len(headers))
    table.style = "Table Grid"
    table.autofit = True
    for index, text in enumerate(headers):
        cell = table.rows[0].cells[index]
        cell.text = text
        set_cell_shading(cell, "1F4E79")
        for run in cell.paragraphs[0].runs:
            run.font.color.rgb = RGBColor(255, 255, 255)
            run.bold = True
    repeat_header_row(table.rows[0])
    for values in rows:
        cells = table.add_row().cells
        for index, text in enumerate(values):
            cells[index].text = text
    for row in table.rows:
        keep_row_together(row)
        for cell in row.cells:
            set_cell_border(cell)
            for paragraph in cell.paragraphs:
                paragraph.paragraph_format.space_after = Pt(3)
                for run in paragraph.runs:
                    run.font.size = Pt(9.5)
    doc.add_paragraph()


def add_heading(doc: Document, text: str, level: int = 1) -> None:
    paragraph = doc.add_heading(text, level=level)
    for run in paragraph.runs:
        run.font.color.rgb = RGBColor(0, 0, 0)


def add_bullet(doc: Document, text: str) -> None:
    doc.add_paragraph(text, style="List Bullet")


def configure(doc: Document, footer_label: str) -> None:
    section = doc.sections[0]
    section.page_width = Inches(8.5)
    section.page_height = Inches(11)
    section.top_margin = Inches(0.8)
    section.bottom_margin = Inches(0.75)
    section.left_margin = Inches(0.8)
    section.right_margin = Inches(0.8)
    styles = doc.styles
    styles["Normal"].font.name = "Aptos"
    styles["Normal"].font.size = Pt(10.5)
    styles["Normal"]._element.rPr.rFonts.set(qn("w:ascii"), "Aptos")
    styles["Normal"]._element.rPr.rFonts.set(qn("w:hAnsi"), "Aptos")
    for name in ("Title", "Heading 1", "Heading 2"):
        styles[name].font.color.rgb = RGBColor(0, 0, 0)
        styles[name].font.name = "Aptos Display"
    styles["Title"].font.size = Pt(28)
    styles["Title"].paragraph_format.space_before = Pt(0)
    styles["Title"].paragraph_format.space_after = Pt(10)
    remove_style_borders(styles["Title"])
    footer = section.footer.paragraphs[0]
    footer.alignment = WD_ALIGN_PARAGRAPH.CENTER
    footer.add_run(f"{footer_label} - Generated non-canonical artifact")
    footer.runs[0].font.size = Pt(8)


def add_title(doc: Document, title_text: str, revision_date: str) -> None:
    version = product_version()
    title = doc.add_paragraph(title_text, style="Title")
    remove_paragraph_borders(title)
    title.alignment = WD_ALIGN_PARAGRAPH.CENTER
    subtitle = doc.add_paragraph()
    subtitle.alignment = WD_ALIGN_PARAGRAPH.CENTER
    subtitle.add_run(f"Product version {version}  |  Document revision {revision_date}").bold = True
    scope = doc.add_paragraph()
    scope.alignment = WD_ALIGN_PARAGRAPH.CENTER
    scope.add_run("Generated from canonical product metadata, ADRs and the versioned project handbook.")
    doc.add_paragraph()
    doc.add_paragraph(
        "This technical edition distinguishes implemented M0.5 foundation behavior from accepted "
        "architecture and planned work. It is a generated distribution artifact, not an independent "
        "source of product truth."
    )


def build_overview(output: Path, revision_date: str) -> None:
    doc = Document()
    configure(doc, "MAIA Technical Overview v1.9")
    add_title(doc, "MAIA Technical Overview", revision_date)

    add_heading(doc, "Product vision")
    doc.add_paragraph(
        "MAIA is a potentially commercial, universal, local-first intelligent work system. Its aim is "
        "persistent and safe assistance around work context, communication, projects, people, commitments "
        "and decisions. The product remains useful with external providers disabled, while allowing "
        "controlled consultation when policy permits it."
    )

    add_heading(doc, "Implementation status")
    add_table(doc, ["Area", "Current status"], [
        ["Implemented foundation", "M0.5 provides OS-neutral domain, policy, orchestration, authoritative SQLite persistence, runtime composition, a one-shot executor boundary, explicit outcome handling and audit foundations."],
        ["Accepted architecture", "Local-first provider neutrality, human approval, privacy/egress control, replaceable models, Intelligence Fabric direction and Round Table boundary."],
        ["Planned or deferred", "Concrete connectors, scheduler loops, model providers, user interface, workspace profiles, persistent people/project context, Case Dossier and product packaging."],
    ])

    add_heading(doc, "Canonical architecture and runtime")
    doc.add_paragraph(
        "Canonical machine-readable contracts live in spec files; ADRs explain accepted boundaries. "
        "The domain, policy and orchestration layers remain OS-neutral. The store contract is implemented "
        "by the SQLite adapter, while runtime composes transactional use cases. The executor is a bounded "
        "one-shot port: it revalidates, claims, invokes one injected executor and persists a classified result."
    )
    add_bullet(doc, "Task, Plan, Action, Approval and Run are distinct aggregates with explicit identity and version semantics.")
    add_bullet(doc, "Approval binds the exact action revision, hash, policy snapshot and assurance; current policy is checked again before execution.")
    add_bullet(doc, "A non-idempotent unknown outcome is reconciled before a semantically equivalent effect may be retried.")

    add_heading(doc, "Persistence and audit")
    doc.add_paragraph(
        "One local workspace has one authoritative mutable store and writable authority process. SQLite owns "
        "migrations, locking, compare-and-swap behavior, evidence and append-only audit-chain writes. Every "
        "authority mutation includes its audit append in the same transaction. Provider chat history is not "
        "canonical MAIA memory."
    )

    add_heading(doc, "Local first development model")
    doc.add_paragraph(
        "MAIA-LOCAL is intended for bounded local development work and MAIA-FRONTIER for architecture, "
        "security and final review. In this edition, the isolated Aider qualification failed its read-only "
        "canary because it modified a disposable fixture. No local worktree or repository contribution was "
        "created. This evidence is a tooling gate, not an implemented product capability."
    )

    add_heading(doc, "Intelligence Fabric and ConsultationPacket")
    doc.add_paragraph(
        "The accepted direction is a provider-neutral Intelligence Fabric. Local, subscription-backed, metered "
        "and frontier models are replaceable computational or consultation resources. A future ConsultationPacket "
        "will reduce and purpose-bind evidence before external consultation. Capability discovery, credential "
        "handling, health accounting and automatic routing are deferred pending canonical contracts."
    )

    add_heading(doc, "Privacy egress and controlled actions")
    doc.add_paragraph(
        "External consultation must cross explicit privacy, policy and approval boundaries. Context is minimized, "
        "redacted where needed and retained with provenance. MAIA never permits hidden model or connector fallback. "
        "Consequential effects require approval, policy revalidation, exact binding, audit and reconciliation."
    )

    add_heading(doc, "Round Table and Case Dossier")
    doc.add_paragraph(
        "Round Table is a planned bounded multi-model consultation subsystem with explicit roles, limited rounds, "
        "conclusions and dissent. A future Case Dossier will support factual reconstruction independent of provider "
        "session history. Neither subsystem is implemented in M0.5 and neither replaces MAIA's authority, memory, "
        "audit or approval model."
    )

    add_heading(doc, "Universal user and workspace model")
    doc.add_paragraph(
        "MAIA has one common core. Personal, professional/executive, technical/knowledge-worker and team/enterprise "
        "profiles compose capabilities, connectors, policy, roles and surfaces without forking core behavior. Enterprise "
        "policy can tighten constraints but cannot silently broaden egress or authority."
    )

    add_heading(doc, "Commercial readiness")
    doc.add_paragraph(
        "Commercial readiness is an engineering practice: every prospective reuse requires immutable provenance, license "
        "obligation, security, operational-cost and coupling review. This documentation references comparative research on "
        "existing agentic frameworks without making that benchmark central to public technical documentation."
    )

    add_heading(doc, "High level roadmap")
    add_table(doc, ["Sequence", "Scope"], [
        ["Completed", "M0.0 through M0.5 headless foundation and accepted Project Knowledge Foundation."],
        ["Current documentation iteration", "Forward product direction, benchmark research, commercial register, deterministic publication and portable export."],
        ["Next proposal", "Architecture Desk review for the shortest safe visible-product milestone; no milestone is authorized by this document."],
    ])
    doc.add_paragraph(
        "Source authority: spec files, ADRs, generated artifacts, then project handbook. Regenerate this document; do not edit it as canonical truth."
    )
    add_heading(doc, "Publication and source discipline")
    doc.add_paragraph(
        "This edition is deliberately separate from the MAIA product version. It is published from versioned "
        "sources and carries its own revision date so documentation maintenance never creates a product-version bump. "
        "The portable Library export contains this DOCX and PDF only as generated, non-canonical documentation."
    )
    add_bullet(doc, "Do not infer an implemented connector, provider, user interface or scheduler from a target-architecture section.")
    add_bullet(doc, "Do not treat comparative research as source-reuse authorization or a substitute for a future architecture decision.")
    add_bullet(doc, "Return the next product-milestone proposal to Architecture Desk before implementing product capabilities beyond the accepted foundation.")
    output.parent.mkdir(parents=True, exist_ok=True)
    doc.save(output)


def build_full(output: Path, revision_date: str) -> None:
    """Publish the fuller technical edition from accepted source material."""
    doc = Document()
    configure(doc, "MAIA Technical Documentation v1.9")
    add_title(doc, "MAIA Technical Documentation", revision_date)

    add_heading(doc, "Scope and authority hierarchy")
    doc.add_paragraph(
        "MAIA is a potentially commercial, universal, local-first intelligent work system. "
        "This document explains the M0.5 foundation and its accepted direction without promoting future "
        "architecture into present capability. Authority is strict: canonical spec files first, accepted ADRs "
        "second, generated artifacts third, then narrative documentation and implementation evidence."
    )
    add_bullet(doc, "Canonical contracts: spec files define fields, state machines, policy and security semantics.")
    add_bullet(doc, "Accepted rationale: ADRs describe boundaries and consequences without replacing canonical YAML.")
    add_bullet(doc, "Generated documentation: this DOCX/PDF is a non-canonical publication and must be regenerated, not hand-edited as truth.")

    add_heading(doc, "Implementation status")
    add_table(doc, ["Classification", "Meaning at product version 1.4.0"], [
        ["IMPLEMENTED", "M0.5 headless foundation: domain contracts, policy/orchestration, authoritative SQLite store, runtime coordinator, one-shot executor boundary and audit foundations."],
        ["ACCEPTED ARCHITECTURE", "Local-first provider neutrality, controlled action, privacy/egress, persona separation, Intelligence Fabric and Round Table boundaries."],
        ["PLANNED / DEFERRED", "Concrete connectors, scheduler/worker loops, user interfaces, provider adapters, persistent work context, Case Dossier and product packaging."],
    ])

    add_heading(doc, "Domain entities and state machines")
    doc.add_paragraph(
        "AgentTask, ExecutionPlan, Action, Approval and Run are distinct aggregates. A plan and action revision are immutable; "
        "an approval binds an exact action id, version and hash; a Run binds one action revision and one approval version. "
        "Mutable task and run records have separate compare-and-swap versions."
    )
    add_table(doc, ["Machine", "Implemented safety semantics"], [
        ["AgentTask", "Draft through preflight, approval/queue and running states; pause is quiescent and non-preemptive: running to pausing to paused only after in-flight work resolves."],
        ["Action", "Planned, gated, approval/queue, running, reconciliation and terminal states; a material edit creates a new revision."],
        ["Approval", "Pending can become approved, rejected, expired or revoked; an approved decision can be revoked."],
        ["Run", "One execution attempt: created, starting, running, terminal result or outcome_unknown/reconciling. Retryable error is terminal for that Run and retry uses a new RunId."],
    ])

    add_heading(doc, "Authorization and approvals")
    doc.add_paragraph(
        "Policy decides whether resolved facts authorize an operation. Approval records a human decision and assurance level; it is not a blanket grant. "
        "Before execution MAIA rechecks current policy, actor authorization, exact action binding, approval freshness, routing and required evidence. "
        "External sends, destructive actions and privileged operations pass ApprovalGate under canonical policy."
    )

    add_heading(doc, "Persistence audit and transaction boundaries")
    doc.add_paragraph(
        "One workspace has one authoritative mutable store and one writable authority process. The SQLite adapter owns migrations, WAL behavior, authority locking, CAS, evidence and append-only audit-chain writes. "
        "Every authority mutation appends its audit record in the same transaction. T1 creates an action revision and related approval/audit work; T2 decides an approval by CAS; T3 creates a Run; T4 transitions a Run with evidence; T5 requests task pause."
    )

    add_heading(doc, "Executor reconciliation recovery and freshness")
    doc.add_paragraph(
        "The executor is an OS-neutral, one-shot boundary. It loads authoritative material, revalidates eligibility, atomically claims created to starting and starting to running, invokes one injected executor port, then persists a classified result. "
        "It owns no connector, scheduler, filesystem or network implementation. An unknown non-idempotent outcome is never silently retried: reconciliation must establish the prior effect. Immediately before a material effect, source freshness must be re-read or conditionally validated."
    )

    add_heading(doc, "Security credentials and connector boundaries")
    doc.add_paragraph(
        "Untrusted mail, Teams, documents and tool output remain data rather than instructions. MAIA uses least privilege, typed approval rendering, provenance, model/connector identity visibility and no hidden fallback. "
        "Credential Vault direction is OS-native: SQLite stores only non-secret references and metadata; secret material belongs in an OS keyring or enterprise vault. Plaintext secrets must not appear in SQLite, logs, exports, fixtures or prompts."
    )
    doc.add_paragraph(
        "Concrete connectors remain deferred. The core and executor are OS-neutral; connector adapters, external side-effect implementations and scheduler workers are outside those crates and must preserve approval, policy, audit, privacy and reconciliation boundaries."
    )

    add_heading(doc, "Local first and development harnesses")
    doc.add_paragraph(
        "MAIA remains useful with external providers disabled. MAIA-LOCAL is a bounded local development path, while MAIA-FRONTIER retains architecture, security and final review. "
        "The Aider candidate failed qualification because its natural-language read-only instruction did not prevent a fixture edit. This confirms that a prompt is not a permission boundary. No MAIA-LOCAL worktree or repository contribution was created."
    )

    add_heading(doc, "Intelligence Fabric ConsultationPacket and privacy egress")
    doc.add_paragraph(
        "The accepted Intelligence Fabric direction treats local, subscription-backed, metered and frontier models as replaceable compute or consultation resources. "
        "A future ConsultationPacket will be a reduced, purpose-bound evidence package. External consultation must minimize and redact context as required, retain provenance and pass privacy, policy and approval boundaries. Provider session history never becomes canonical MAIA memory."
    )

    add_heading(doc, "Round Table and Case Dossier")
    doc.add_paragraph(
        "Round Table is a planned reusable, bounded multi-model consultation subsystem with explicit roles, limited rounds, conclusions and dissent. "
        "A planned Case Dossier will reconstruct long-running factual cases independent of provider history. Neither feature is implemented in M0.5 and neither may replace local authority, audit, policy or human approval."
    )

    add_heading(doc, "Universal users workspaces and persona")
    doc.add_paragraph(
        "One common MAIA core serves personal, professional/executive, technical/knowledge-worker and team/enterprise profiles by composing capabilities, connector adapters, policy, roles and surfaces. "
        "Enterprise policy may tighten constraints but cannot silently broaden egress or authority."
    )
    doc.add_paragraph(
        "MAIA Core has no gender. A separately configured MAIA-owned presentation layer has a default feminine persona: intelligent, competent, calm, warm, professional and lightly witty when appropriate. "
        "It may influence wording and presentation, but never truth, risk, approval, permissions, privacy, egress, audit, execution authorization, evidence or connector authority. Persona Engine, UI, voice and avatar remain deferred."
    )

    add_heading(doc, "Project knowledge commercial readiness and roadmap")
    doc.add_paragraph(
        "Project knowledge is reconstructable from canonical specs, ADRs, handbook sources and deterministic generated artifacts, not from AI chat history. The shared mirror and portable Library export are generated from the handbook. "
        "Commercial readiness requires provenance, license obligations, security review, distribution classification, cost and coupling analysis before a component becomes a dependency. Comparative research informs choices but is not source-reuse authorization."
    )
    add_table(doc, ["History", "Accepted result"], [
        ["M0", "Bootstrapped the canonical MAIA repository and handoff workflow."],
        ["M0.0.1", "Closed canonical execution contracts."],
        ["M0.1", "Established generated Rust canonical domain foundation."],
        ["M0.1.1", "Closed execution binding and authorization contract gaps."],
        ["M0.1.2", "Hardened approval and execution conformance."],
        ["M0.2", "Added pure execution policy and orchestration semantics."],
        ["M0.2.1", "Closed authoritative persistence and audit contracts."],
        ["M0.3", "Added authoritative local store and SQLite adapter."],
        ["M0.4", "Added the transactional runtime coordinator."],
        ["M0.5", "Added the one-shot executor boundary and outcome recovery semantics."],
        ["M0.8-M0.10", "Added bounded multi-model consultation, assurance routing and a loopback-only local evidence briefing with immutable persistence."],
        ["M0.11 Alpha 1", "Adds the first native local desktop surface for explicit evidence snapshots, validated local briefings, citation inspection and restart-safe history; no connector, cloud fallback or action path."],
    ])
    add_bullet(doc, "M0.11 does not infer a connector, cloud provider fallback, scheduler, action, Decision, Commitment or WorkGraph promotion.")
    add_bullet(doc, "Any later external effect remains subject to canonical policy, approval, audit, privacy and reconciliation boundaries.")
    output.parent.mkdir(parents=True, exist_ok=True)
    doc.save(output)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--overview-output", type=Path, default=OVERVIEW_OUTPUT)
    parser.add_argument("--full-output", type=Path, default=FULL_OUTPUT)
    parser.add_argument("--revision-date", default=dt.date.today().isoformat())
    args = parser.parse_args()
    build_overview(args.overview_output.resolve(), args.revision_date)
    build_full(args.full_output.resolve(), args.revision_date)
    print(args.overview_output.resolve())
    print(args.full_output.resolve())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
