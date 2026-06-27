import React from 'react';
import {
  FolderCode,
  Gauge,
  History as HistoryIcon,
  MessageCircleQuestionMark,
  Settings2,
  X
} from 'lucide-react';
import claudeCodeIcon from '../assets/claude-code-icon.svg';
import codexAppIcon from '../assets/codex-app-icon.png';
import codexCliIcon from '../assets/codex-cli-icon.png';
import { closeOnBackdropClick } from '../modalEvents.js';

export function AgentIconStack({ agents = [], emptyLabel = 'No installed agent target', labelPrefix = 'Installed agents' }) {
  const visibleAgents = agents.slice(0, 4);
  const overflowCount = Math.max(agents.length - visibleAgents.length, 0);
  const overflowLabel = overflowCount
    ? agents
        .slice(visibleAgents.length)
        .map((agent) => agent.label)
        .join(', ')
    : '';
  const label = agents.length
    ? `${labelPrefix}: ${agents.map((agent) => agent.label).join(', ')}`
    : emptyLabel;

  return (
    <span className="skillAgentIcons" aria-label={label}>
      {visibleAgents.map((agent) => (
        <AgentIconBadge agent={agent} key={agent.id} />
      ))}
      {overflowCount > 0 ? (
        <span className="skillAgentIcon overflow" data-tooltip={overflowLabel} aria-label={overflowLabel}>
          +{overflowCount}
        </span>
      ) : null}
    </span>
  );
}

export function AgentIconBadge({ agent }) {
  if (!agent) {
    return null;
  }

  const iconClass = agentIconClass(agent);
  const iconSource = agentIconSource(agent, iconClass);

  return (
    <span className={`skillAgentIcon ${iconClass}`} data-tooltip={agent.label} aria-label={agent.label}>
      {iconSource ? (
        <img src={iconSource} alt="" aria-hidden="true" />
      ) : (
        <span aria-hidden="true">{agent.iconLabel || agentInitial(agent)}</span>
      )}
    </span>
  );
}

function agentIconClass(agent) {
  return agent.iconClass || agent.id || 'local';
}

function agentIconSource(agent, iconClass = '') {
  if (agent.iconAsset === 'codex-app' || iconClass === 'codex-app' || agent.id === 'codex') {
    return codexAppIcon;
  }
  if (agent.iconAsset === 'codex-cli' || iconClass === 'codex-cli' || agent.id === 'agents') {
    return codexCliIcon;
  }
  if (agent.iconAsset === 'claude-code' || iconClass === 'claude-code' || agent.id === 'claude-code') {
    return claudeCodeIcon;
  }
  return null;
}

function agentInitial(agent) {
  if (agent.id === 'claude-code') return 'CC';
  return String(agent.label || agent.id || '?').slice(0, 1).toUpperCase();
}

export function PageHeader({ actions, eyebrow, subtitle, title }) {
  return (
    <header className="pageHeader">
      <div>
        <p className="eyebrow">{eyebrow}</p>
        <h1>{title}</h1>
        <p>{subtitle}</p>
      </div>
      {actions ? <div className="headerActions">{actions}</div> : null}
    </header>
  );
}

export function PageTitleRow({ actions, count, title }) {
  const hasCount = count !== undefined && count !== null;

  return (
    <div className="pageTitleRow">
      <div className="pageTitleGroup">
        <h1>{title}</h1>
        {hasCount ? <span className="pageTitlePill">{count}</span> : null}
      </div>
      {actions ? <div className="pageTitleActions">{actions}</div> : null}
    </div>
  );
}

export function ConfirmDialog({
  cancelLabel = 'Cancel',
  children,
  className = '',
  closeLabel = 'Close confirmation',
  confirmDisabled = false,
  confirmLabel = 'Confirm',
  description,
  error = '',
  loading = false,
  loadingLabel = 'Working...',
  onClose,
  onConfirm,
  title,
  titleId
}) {
  const dialogClassName = className ? `confirmDialog ${className}` : 'confirmDialog';

  return (
    <div
      className="modalBackdrop"
      role="presentation"
      onMouseDown={(event) => closeOnBackdropClick(event, onClose)}
    >
      <section className={dialogClassName} role="dialog" aria-modal="true" aria-labelledby={titleId}>
        <div className="confirmDialogHeader">
          <div>
            <h2 id={titleId}>{title}</h2>
            {description ? <p>{description}</p> : null}
          </div>
          <button className="iconButton" disabled={loading} type="button" aria-label={closeLabel} onClick={onClose}>
            <X aria-hidden="true" />
          </button>
        </div>

        <div className="confirmDialogBody">
          {children}
          {error ? <div className="formError">{error}</div> : null}
        </div>

        <div className="confirmDialogFooter">
          <button className="button secondary" disabled={loading} type="button" onClick={onClose}>
            {cancelLabel}
          </button>
          <button className="button primary" disabled={loading || confirmDisabled} type="button" onClick={onConfirm}>
            {loading ? (
              <>
                <span className="buttonSpinner" aria-hidden="true" />
                {loadingLabel}
              </>
            ) : (
              confirmLabel
            )}
          </button>
        </div>
      </section>
    </div>
  );
}

export function NavButton({ active, icon, label, onClick }) {
  return (
    <button className={active ? 'navButton active' : 'navButton'} type="button" onClick={onClick}>
      <span className="navIcon">
        <Icon name={icon} />
      </span>
      {label}
    </button>
  );
}

export function FooterButton({ active = false, icon, label, onClick }) {
  return (
    <button className={active ? 'active' : ''} type="button" onClick={onClick}>
      <span className="footerIcon">
        <Icon name={icon} />
      </span>
      {label}
    </button>
  );
}

export function Icon({ name }) {
  if (name === 'gauge') {
    return <Gauge aria-hidden="true" />;
  }

  if (name === 'folder-code') {
    return <FolderCode aria-hidden="true" />;
  }

  if (name === 'history') {
    return <HistoryIcon aria-hidden="true" />;
  }

  if (name === 'settings-2' || name === 'settings') {
    return <Settings2 aria-hidden="true" />;
  }

  if (name === 'message-circle-question-mark' || name === 'help') {
    return <MessageCircleQuestionMark aria-hidden="true" />;
  }

  if (name === 'setup') {
    return (
      <svg aria-hidden="true" viewBox="0 0 24 24">
        <path d="M5 5h14v14H5z" />
        <path d="m9 12 2 2 4-5" />
      </svg>
    );
  }

  if (name === 'dashboard') {
    return (
      <svg aria-hidden="true" viewBox="0 0 24 24">
        <path d="M4 5h7v7H4z" />
        <path d="M13 5h7v4h-7z" />
        <path d="M13 11h7v8h-7z" />
        <path d="M4 14h7v5H4z" />
      </svg>
    );
  }

  if (name === 'workspaces') {
    return (
      <svg aria-hidden="true" viewBox="0 0 24 24">
        <path d="M4 5h6v6H4z" />
        <path d="M14 5h6v6h-6z" />
        <path d="M4 15h6v4H4z" />
        <path d="M14 15h6v4h-6z" />
        <path d="M10 8h4" />
        <path d="M10 17h4" />
      </svg>
    );
  }

  if (name === 'user-skills') {
    return (
      <svg aria-hidden="true" viewBox="0 0 24 24">
        <path d="M16 19a4 4 0 0 0-8 0" />
        <circle cx="12" cy="8" r="3" />
        <path d="M4 5h2" />
        <path d="M18 5h2" />
        <path d="M4 12h2" />
        <path d="M18 12h2" />
      </svg>
    );
  }

  if (name === 'remote-skills') {
    return (
      <svg aria-hidden="true" viewBox="0 0 24 24">
        <circle cx="12" cy="12" r="8" />
        <path d="M4 12h16" />
        <path d="M12 4a12 12 0 0 1 0 16" />
        <path d="M12 4a12 12 0 0 0 0 16" />
      </svg>
    );
  }

  return (
    <svg aria-hidden="true" viewBox="0 0 24 24">
      <circle cx="12" cy="12" r="9" />
      <path d="M9.8 9a2.4 2.4 0 0 1 4.5 1.2c0 1.7-2.1 2-2.1 3.5" />
      <path d="M12 17h.01" />
    </svg>
  );
}

export function Badge({ children, tone = 'slate' }) {
  return <span className={`badge ${tone}`}>{children}</span>;
}

export function PathList({ items }) {
  return (
    <dl className="pathList">
      {items.map(([label, value]) => (
        <div key={label}>
          <dt>{label}</dt>
          <dd>{value || 'Not available'}</dd>
        </div>
      ))}
    </dl>
  );
}

export function LoadingNotice({ children, compact = false }) {
  return (
    <div className={compact ? 'loadingNotice compact' : 'loadingNotice'} aria-live="polite">
      <span className="inlineSpinner" aria-hidden="true" />
      <span>{children}</span>
    </div>
  );
}
