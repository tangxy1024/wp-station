import React, { useEffect, useState } from 'react';
import { App as AntdApp, Table } from 'antd';
import { useTranslation } from 'react-i18next';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { oneDark } from 'react-syntax-highlighter/dist/esm/styles/prism';
import CodeEditor from '@/views/components/CodeEditor';
import {
  parseWfusionRuleEditor,
  wflCodeFormat,
  wfsCodeFormat,
} from '@/services/debug';

const STORAGE_KEYS = {
  events: 'wfusion-rule-editor.events',
  wfs: 'wfusion-rule-editor.wfs',
  wfl: 'wfusion-rule-editor.wfl',
};

const SAMPLE_WFS = `window xy_system_ssh_log {
    stream_tag = "xy_system_ssh_log"
    time = occur_time
    over = 2h
    fields {
        tenant_id: chars
        event_id: chars
        log_id: chars
        raw_log_ref: chars
        occur_time: time
        log_type: chars
        event_category: chars
        event_type: chars
        source_ip: ip
        source_port: digit
        target_ip: chars
        target_host: chars
        target_port: digit
        target_user: chars
        operation: chars
        outcome: chars
        severity: chars
        carrier_protocol: chars
        carrier_process_name: chars
        carrier_process_pid: chars
        observer_product: chars
        whitelist_hit: chars
    }
}

window other_logs {
    stream_tag = [
        "xy_system_audit_log",
        "xy_system_network_log",
        "xy_system_sys_log",
        "xy_system_kernel_log",
        "xy_system_auth_log",
        "xy_system_login_log",
        "xy_nginx_fluent_bit_log",
        "sec_portal_peers_log",
        "sec_portal_users_log",
        "sec_portal_event_log"
    ]
    time = occur_time
    over = 2h
    fields {
        tenant_id: chars
        event_id: chars
        log_id: chars
        raw_log_ref: chars
        occur_time: time
        log_type: chars
        event_category: chars
        event_type: chars
        source_ip: ip
        source_port: digit
        target_ip: chars
        target_host: chars
        target_port: digit
        target_user: chars
        operation: chars
        outcome: chars
        severity: chars
        carrier_protocol: chars
        carrier_process_name: chars
        carrier_process_pid: chars
        observer_product: chars
        whitelist_hit: chars
    }
}

window security_alerts {
    over = 0
    fields {
        alert_id: chars
        alert_display_id: chars
        tenant_id: chars
        merge_id: chars
        source_systems: array/chars
        alert_name: chars
        description: chars
        alert_type: chars
        category_code: chars
        category_name: chars
        subcategory_code: chars
        subcategory_name: chars
        summary: chars
        event_count: chars
        severity: chars
        risk_score: chars
        confidence: chars
        workflow_status: chars
        disposition_status: chars
        ticket_status: chars
        verdict: chars
        compromise_status: chars
        created_time: chars
        updated_time: chars
        first_seen: chars
        last_seen: chars
        evidence_start_time: chars
        evidence_end_time: chars
        duration_seconds: chars
        rule_id: chars
        rule_name: chars
        rule_version: chars
        rule_type: chars
        detection_engine: chars
        primary_entity_id: chars
        primary_entity_type: chars
        primary_entity_value: chars
        dedup_key: chars
        correlation_id: chars
        source_ip: ip
        target_host: chars
        target_user: chars
        rule_window_start: chars
        rule_window_end: chars
        threshold: chars
        distinct_source_port_count: chars
        whitelist_hit: chars
        source_alert_ref: chars
        raw_alert_ref: chars
        latest_analysis_conclusion: chars
        latest_analysis_summary: chars
        latest_analysis_time: chars
        assignee_id: chars
        ticket_id: chars
        incident_ids: chars
        detail: chars
        extensions: chars
    }
}`;

const SAMPLE_WFL = `use "auth.wfs"

rule ssh_brute_force_alert {
  events {
    s : xy_system_ssh_log
      && event_category == "auth"
      && event_type == "ssh_session"
      && operation in ("failed_login", "authenticate")
      && outcome == "failed"
      && observer_product == "sshd"
      && isnotnull(source_ip)
      && is_blank(target_host) == false
  }

  match<tenant_id,source_ip,target_host,target_user:1m:fixed> {
    on event { s | count >= 1; }
  } -> score(
    if count(s) >= 1000 then 100.0
    else if count(s) >= 500 then 80.0
    else if lower(default_if_blank(s.target_user, "unknown")) in ("root", "admin", "administrator") then 73.0
    else 65.0
  )

  entity(ip, s.source_ip)

  yield security_alerts (
    alert_id = concat(
      "alert_",
      sha1(
        fmt(
          "{}|{}|{}|{}|{}|{}",
          s.tenant_id,
          "SDM-SSH-BRUTE-FORCE-001",
          s.source_ip,
          lower(s.target_host),
          lower(default_if_blank(s.target_user, "unknown")),
          strftime(now(), "%Y-%m-%d %H:%M:%S%.3f")
        )
      )
    ),
    alert_display_id = concat(
      "ALERT-",
      sha1(
        fmt(
          "{}|{}|{}|{}|{}|{}",
          s.tenant_id,
          "SDM-SSH-BRUTE-FORCE-001",
          s.source_ip,
          lower(s.target_host),
          lower(default_if_blank(s.target_user, "unknown")),
          strftime(now(), "%Y-%m-%d %H:%M:%S%.3f")
        )
      )
    ),
    tenant_id = s.tenant_id,
    merge_id = concat(
      "merge_",
      sha1(
        fmt(
          "{}|{}|{}|{}|{}",
          s.tenant_id,
          "SDM-SSH-BRUTE-FORCE-001",
          s.source_ip,
          lower(s.target_host),
          lower(default_if_blank(s.target_user, "unknown"))
        )
      )
    ),
    source_systems = split("sdm-rule-engine", ","),
    alert_name = fmt(
      "SSH 暴力破解/撞库 - {} -> {}@{}",
      s.source_ip,
      default_if_blank(s.target_user, "unknown"),
      s.target_host
    ),
    description = fmt(
      "{} 在 {} 命中 SSH 失败登录阈值，对 {} 的 {} 账号已达到 1 分钟 3 次及以上失败。",
      s.source_ip,
      strftime(now(), "%Y-%m-%d %H:%M:%S%.3f"),
      s.target_host,
      default_if_blank(s.target_user, "unknown")
    ),
    alert_type = "DETECTION",
    category_code = "intrusion",
    category_name = "intrusion",
    subcategory_code = "ssh_bruteforce",
    subcategory_name = "SSH 暴力破解/撞库",
    summary = fmt(
      "SSH 登录失败达到阈值，来源 {}，目标 {}@{}。",
      s.source_ip,
      default_if_blank(s.target_user, "unknown"),
      s.target_host
    ),
    event_count = "3",
    severity = if count(s) >= 1000 then "CRITICAL" else if count(s) >= 500 then "HIGH" else "MEDIUM",
    risk_score = fmt("{}", if lower(default_if_blank(s.target_user, "unknown")) in ("root", "admin", "administrator") then @score + 8.0 else @score),
    confidence = fmt("{}", if lower(default_if_blank(s.target_user, "unknown")) in ("root", "admin", "administrator") then 85.0 else 80.0),
    workflow_status = "NEW",
    disposition_status = "pending",
    ticket_status = "",
    verdict = "SUSPICIOUS",
    compromise_status = "",
    created_time = strftime(now(), "%Y-%m-%d %H:%M:%S%.3f"),
    updated_time = strftime(now(), "%Y-%m-%d %H:%M:%S%.3f"),
    first_seen = strftime(now(), "%Y-%m-%d %H:%M:%S%.3f"),
    last_seen = strftime(now(), "%Y-%m-%d %H:%M:%S%.3f"),
    evidence_start_time = strftime(now(), "%Y-%m-%d %H:%M:%S%.3f"),
    evidence_end_time = strftime(now(), "%Y-%m-%d %H:%M:%S%.3f"),
    duration_seconds = "0",
    rule_id = "SDM-SSH-BRUTE-FORCE-001",
    rule_version = "1.0.0",
    rule_type = "THRESHOLD",
    detection_engine = "sdm-rule-engine",
    primary_entity_id = concat("host:", s.tenant_id, ":", sha1(lower(s.target_host))),
    primary_entity_type = "host",
    primary_entity_value = s.target_host,
    dedup_key = fmt(
      "{}|{}|{}|{}|{}|{}",
      s.tenant_id,
      "SDM-SSH-BRUTE-FORCE-001",
      s.source_ip,
      s.target_host,
      default_if_blank(s.target_user, "unknown"),
      strftime(now(), "%Y-%m-%d %H:%M:%S%.3f")
    ),
    correlation_id = "",
    source_ip = s.source_ip,
    target_host = s.target_host,
    target_user = default_if_blank(s.target_user, "unknown"),
    rule_window_start = strftime(now(), "%Y-%m-%d %H:%M:%S%.3f"),
    rule_window_end = strftime(now(), "%Y-%m-%d %H:%M:%S%.3f"),
    threshold = "3",
    distinct_source_port_count = if s.source_port > 0 then "1" else "0",
    whitelist_hit = default_if_blank(s.whitelist_hit, "false"),
    source_alert_ref = "",
    raw_alert_ref = "",
    latest_analysis_conclusion = "suspicious",
    latest_analysis_summary = "规则命中：来源 IP 在固定窗口内对同一目标账号产生大量 SSH 认证失败。",
    latest_analysis_time = strftime(now(), "%Y-%m-%d %H:%M:%S%.3f"),
    assignee_id = "",
    ticket_id = "",
    incident_ids = "[]",
    detail = fmt(
      "规则命中：{} 在 {} 对 {}@{} 达到 1 分钟 3 次及以上 SSH 失败登录阈值。",
      s.source_ip,
      strftime(now(), "%Y-%m-%d %H:%M:%S%.3f"),
      default_if_blank(s.target_user, "unknown"),
      s.target_host
    ),
    extensions = fmt(
      "source_ip={};target_host={};target_user={};occur_time={};window_start={};window_end={};threshold={};event_count={};source_port={};whitelist_hit={}",
      s.source_ip,
      s.target_host,
      default_if_blank(s.target_user, "unknown"),
      strftime(now(), "%Y-%m-%d %H:%M:%S%.3f"),
      strftime(now(), "%Y-%m-%d %H:%M:%S%.3f"),
      strftime(now(), "%Y-%m-%d %H:%M:%S%.3f"),
      3,
      3,
      s.source_port,
      default_if_blank(s.whitelist_hit, "false")
    )
  )

  limits {
    max_memory = "64MB";
    max_instances = 10000;
    on_exceed = throttle;
  }
}`;

const SAMPLE_EVENTS = `{"_stream":"xy_system_ssh_log","event_id":"860446468382921265","tenant_id":"tenant01","log_id":"860446468382921265","occur_time":1783911991000000000,"event_type":"ssh_session","event_category":"auth","log_type":"xy_system_ssh_log","source_ip":"220.181.41.82","source_port":56928,"target_user":"wfusion","target_host":"ent-bas-zerotrust-01","target_ip":"","target_port":0,"target_domain":"","target_url":"","target_file_path":"","carrier_protocol":"tcp","http_method":"","carrier_process_name":"sshd","carrier_process_pid":"2982287","carrier_process_path":"","observer_product":"sshd","operation":"failed_login","outcome":"failed","http_status":0,"severity":"info","raw_log_ref":"journald:sshd","whitelist_hit":"false"}`;

const readStorage = (key, fallback = '') => {
  if (typeof window === 'undefined') {
    return fallback;
  }
  try {
    return window.localStorage.getItem(key) || fallback;
  } catch (_error) {
    return fallback;
  }
};

const persistStorage = (key, value) => {
  if (typeof window === 'undefined') {
    return;
  }
  try {
    window.localStorage.setItem(key, value || '');
  } catch (_error) {
    // 忽略浏览器存储失败，不影响主流程
  }
};

const prettyJson = (value) => {
  try {
    return JSON.stringify(value, null, 2);
  } catch (_error) {
    return String(value);
  }
};

const IP_PATTERN =
  /^(?:(?:25[0-5]|2[0-4]\d|1?\d?\d)\.){3}(?:25[0-5]|2[0-4]\d|1?\d?\d)$/;

const isEmptyResultValue = (value) =>
  value === undefined ||
  value === null ||
  value === '' ||
  value === '-' ||
  (Array.isArray(value) && value.length === 0);

const inferResultMeta = (value) => {
  if (isEmptyResultValue(value)) {
    return 'ignore';
  }
  if (typeof value === 'boolean') {
    return 'bool';
  }
  if (typeof value === 'number') {
    return Number.isInteger(value) ? 'digit' : 'float';
  }
  if (Array.isArray(value)) {
    return 'array';
  }
  if (typeof value === 'string') {
    if (IP_PATTERN.test(value)) {
      return 'ip';
    }
    if (/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/.test(value)) {
      return 'time';
    }
    return 'chars';
  }
  return 'json';
};

const formatResultValue = (value) => {
  if (isEmptyResultValue(value)) {
    return '-';
  }
  if (typeof value === 'string') {
    return value;
  }
  return prettyJson(value);
};

const pruneEmptyValues = (value) => {
  if (Array.isArray(value)) {
    return value
      .map(pruneEmptyValues)
      .filter((item) => !isEmptyResultValue(item));
  }

  if (value && typeof value === 'object') {
    return Object.entries(value).reduce((acc, [key, current]) => {
      const pruned = pruneEmptyValues(current);
      if (!isEmptyResultValue(pruned)) {
        acc[key] = pruned;
      }
      return acc;
    }, {});
  }

  return value;
};

const buildAlertTableRows = (alerts = []) => {
  let rowNo = 1;
  return alerts.flatMap((alert, alertIndex) => {
    if (!alert || typeof alert !== 'object' || Array.isArray(alert)) {
      const name = alerts.length > 1 ? `${alertIndex + 1}` : 'value';
      const value = formatResultValue(alert);
      const row = {
        no: rowNo,
        key: `${alertIndex}-${name}`,
        meta: inferResultMeta(alert),
        name,
        value,
      };
      rowNo += 1;
      return [row];
    }

    return Object.entries(alert).map(([name, value]) => {
      const row = {
        no: rowNo,
        key: `${alertIndex}-${name}`,
        meta: inferResultMeta(value),
        name: alerts.length > 1 ? `${alertIndex + 1}.${name}` : name,
        value: formatResultValue(value),
      };
      rowNo += 1;
      return row;
    });
  });
};

const filterAlertRows = (rows = [], showEmpty) =>
  showEmpty ? rows : rows.filter((row) => !isEmptyResultValue(row.value) && row.value !== '-');

const renderResultError = (t, result) => {
  const diagnostics = Array.isArray(result?.diagnostics) ? result.diagnostics : [];
  return (
    <div className="wfusion-rule-editor__error">
      <div className="wfusion-rule-editor__error-header">
        <strong>{t('simulateDebug.parseResult.parseFailed')}</strong>
      </div>
      {diagnostics.length > 0 ? (
        diagnostics.map((item, index) => (
          <div key={`${item.file || 'diagnostic'}-${index}`} className="wfusion-rule-editor__error-item">
            <div>{item.message || t('wfusionRuleEditor.requestFailed')}</div>
            {(item.category || item.file || item.line || item.column) ? (
              <div className="wfusion-rule-editor__error-meta">
                {item.category ? (
                  <span>
                    {t('wfusionRuleEditor.errorCategory')}
                    {item.category}
                  </span>
                ) : null}
                {item.file ? (
                  <span>
                    {t('wfusionRuleEditor.errorFile')}
                    {item.file}
                  </span>
                ) : null}
                {item.line ? (
                  <span>
                    {t('wfusionRuleEditor.errorLocation')}
                    {item.line}
                    {item.column ? `:${item.column}` : ''}
                  </span>
                ) : null}
              </div>
            ) : null}
            {item.rule ? (
              <div className="wfusion-rule-editor__error-meta">
                {t('wfusionRuleEditor.errorRule')}
                {item.rule}
              </div>
            ) : null}
            {item.test ? (
              <div className="wfusion-rule-editor__error-meta">
                {t('wfusionRuleEditor.errorTest')}
                {item.test}
              </div>
            ) : null}
            {item.snippet ? (
              <pre className="wfusion-rule-editor__error-snippet">{item.snippet}</pre>
            ) : null}
            {item.hint ? <div className="wfusion-rule-editor__error-hint">{item.hint}</div> : null}
          </div>
        ))
      ) : (
        <div className="wfusion-rule-editor__error-item">
          {t('wfusionRuleEditor.requestFailed')}
        </div>
      )}
    </div>
  );
};

function WfusionResultContent({ t, result, viewMode, showEmpty }) {
  const alerts = Array.isArray(result?.alerts) ? result.alerts : [];
  const resultRows = filterAlertRows(buildAlertTableRows(alerts), showEmpty);
  const resultJson = prettyJson(showEmpty ? alerts : pruneEmptyValues(alerts));
  const resultColumns = [
    { title: t('simulateDebug.table.no'), dataIndex: 'no', key: 'no', width: 70 },
    { title: t('simulateDebug.table.meta'), dataIndex: 'meta', key: 'meta', width: 140 },
    { title: t('simulateDebug.table.name'), dataIndex: 'name', key: 'name', width: 220 },
    { title: t('simulateDebug.table.value'), dataIndex: 'value', key: 'value' },
  ];

  if (!result) {
    return (
      <div className="wfusion-rule-editor__empty">
        {t('wfusionRuleEditor.noResult')}
      </div>
    );
  }

  if (result.success === false) {
    return renderResultError(t, result);
  }

  return (
    <div className="wfusion-rule-editor__result">
      {viewMode === 'table' ? (
        alerts.length > 0 ? (
          <div style={{ paddingBottom: '10px' }}>
            <Table
              size="small"
              columns={resultColumns}
              dataSource={resultRows}
              pagination={false}
              rowKey="key"
              className="data-table compact"
              scroll={{ y: 460, scrollToFirstRowOnChange: true }}
            />
          </div>
        ) : (
          <div className="wfusion-rule-editor__empty">
            {t('wfusionRuleEditor.noAlerts')}
          </div>
        )
      ) : alerts.length > 0 ? (
        <div className="json-result-scroll">
          <SyntaxHighlighter
            className="code-block"
            language="json"
            style={oneDark}
            customStyle={{
              margin: 0,
              background: '#0f172a',
              width: '100%',
              minWidth: 0,
            }}
            codeTagProps={{ style: { background: 'transparent' } }}
            wrapLines
            lineProps={{ style: { background: 'transparent' } }}
            wrapLongLines
          >
            {resultJson}
          </SyntaxHighlighter>
        </div>
      ) : (
        <div className="wfusion-rule-editor__empty">
          {t('wfusionRuleEditor.noAlerts')}
        </div>
      )}
    </div>
  );
}

export function WfusionRuleEditorContent() {
  const { t } = useTranslation();
  const { message } = AntdApp.useApp();
  const [eventsNdjson, setEventsNdjson] = useState(() => readStorage(STORAGE_KEYS.events, ''));
  const [wfsCode, setWfsCode] = useState(() => readStorage(STORAGE_KEYS.wfs, ''));
  const [wflCode, setWflCode] = useState(() => readStorage(STORAGE_KEYS.wfl, ''));
  const [result, setResult] = useState(null);
  const [activeRuleEditor, setActiveRuleEditor] = useState('wfs');
  const [resultViewMode, setResultViewMode] = useState('table');
  const [showEmpty, setShowEmpty] = useState(true);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    persistStorage(STORAGE_KEYS.events, eventsNdjson);
  }, [eventsNdjson]);

  useEffect(() => {
    persistStorage(STORAGE_KEYS.wfs, wfsCode);
  }, [wfsCode]);

  useEffect(() => {
    persistStorage(STORAGE_KEYS.wfl, wflCode);
  }, [wflCode]);

  const handleFormat = async (kind) => {
    const source = kind === 'wfs' ? wfsCode : wflCode;
    if (!source.trim()) {
      message.warning(t('common.noFormatContent'));
      return;
    }

    try {
      if (kind === 'wfs') {
        const formatted = await wfsCodeFormat(source);
        setWfsCode(formatted?.wfs_code || source);
      } else {
        const formatted = await wflCodeFormat(source);
        setWflCode(formatted?.wfl_code || source);
      }
      message.success(t('ruleManage.format'));
    } catch (error) {
      message.error(
        kind === 'wfs' ? t('debug.wfs.formatError') : t('debug.wfl.formatError'),
      );
    }
  };

  const handleParse = async () => {
    setLoading(true);
    try {
      const nextResult = await parseWfusionRuleEditor({
        eventsNdjson,
        wfs: wfsCode,
        wfl: wflCode,
      });
      setResult(nextResult);
      if (nextResult?.success) {
        message.success(t('wfusionRuleEditor.success'));
      } else {
        message.warning(t('wfusionRuleEditor.failed'));
      }
    } catch (error) {
      setResult({
        success: false,
        stage: 'replay',
        diagnostics: [
          {
            severity: 'error',
            file: 'wfusion',
            message: error?.message || t('wfusionRuleEditor.requestFailed'),
          },
        ],
        alerts: [],
      });
      message.error(error?.message || t('wfusionRuleEditor.requestFailed'));
    } finally {
      setLoading(false);
    }
  };

  const handleLoadExample = () => {
    setEventsNdjson(SAMPLE_EVENTS);
    setWfsCode(SAMPLE_WFS);
    setWflCode(SAMPLE_WFL);
    setResult(null);
  };

  const handleClearAll = () => {
    setEventsNdjson('');
    setWfsCode('');
    setWflCode('');
    setResult(null);
  };

  const activeRuleCode = activeRuleEditor === 'wfs' ? wfsCode : wflCode;
  const activeRuleLanguage = activeRuleEditor === 'wfs' ? 'wfs' : 'wfl';
  const activeRuleTitle =
    activeRuleEditor === 'wfs' ? t('wfusionRuleEditor.wfsTitle') : t('wfusionRuleEditor.wflTitle');
  const resultSummary = result?.success ? result?.summary || null : null;

  const handleActiveRuleChange = (value) => {
    if (activeRuleEditor === 'wfs') {
      setWfsCode(value);
      return;
    }
    setWflCode(value);
  };

  return (
    <div className="wfusion-rule-editor wfusion-rule-editor--compact">
      <div className="panel-block wfusion-rule-editor__log-block">
        <div className="block-header" style={{ flexWrap: 'nowrap', alignItems: 'center' }}>
          <div>
            <h3>{t('wfusionRuleEditor.ndjsonTitle')}</h3>
          </div>
          <div
            className="block-actions"
            style={{ display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'nowrap', minWidth: 0 }}
          >
            <button type="button" className="btn primary" onClick={handleLoadExample}>
              {t('wfusionRuleEditor.loadExample')}
            </button>
            <button type="button" className="btn ghost" onClick={handleClearAll}>
              {t('wfusionRuleEditor.clearAll')}
            </button>
          </div>
        </div>
        <CodeEditor
          className="code-area code-area--log-input wfusion-rule-editor__log-input"
          language="json"
          theme="vscodeDark"
          value={eventsNdjson}
          onChange={setEventsNdjson}
        />
      </div>

      <div className="split-layout wfusion-rule-editor__workspace">
        <div className="split-col wfusion-rule-editor__editor-col">
          <div className="panel-block panel-block--fill">
            <div className="block-header wfusion-rule-editor__editor-header">
              <div className="wfusion-rule-editor__editor-title">
                <h3>{activeRuleTitle}</h3>
                <div className="mode-toggle wfusion-rule-editor__editor-toggle">
                  <button
                    type="button"
                    className={`toggle-btn ${activeRuleEditor === 'wfs' ? 'is-active' : ''}`}
                    onClick={() => setActiveRuleEditor('wfs')}
                  >
                    {t('wfusionRuleEditor.wfsTitle')}
                  </button>
                  <button
                    type="button"
                    className={`toggle-btn ${activeRuleEditor === 'wfl' ? 'is-active' : ''}`}
                    onClick={() => setActiveRuleEditor('wfl')}
                  >
                    {t('wfusionRuleEditor.wflTitle')}
                  </button>
                </div>
              </div>
              <div className="block-actions wfusion-rule-editor__editor-actions">
                <button
                  type="button"
                  className="btn ghost"
                  onClick={() => handleFormat(activeRuleEditor)}
                >
                  {t('ruleManage.format')}
                </button>
                <button
                  type="button"
                  className="btn primary"
                  onClick={handleParse}
                  disabled={loading}
                >
                  {loading ? t('wfusionRuleEditor.parsing') : t('wfusionRuleEditor.parse')}
                </button>
              </div>
            </div>
            <CodeEditor
              className="code-area code-area--large wfusion-rule-editor__rule-input"
              language={activeRuleLanguage}
              theme="vscodeDark"
              value={activeRuleCode}
              onChange={handleActiveRuleChange}
            />
          </div>
        </div>

        <div className="split-col wfusion-rule-editor__result-col">
          <div className="panel-block panel-block--stretch panel-block--scrollable wfusion-rule-editor__result-panel">
            <div className="wfusion-rule-editor__result-header">
              <div className="wfusion-rule-editor__result-title">
                <h3>{t('simulateDebug.parseResult.title')}</h3>
              </div>
              <div className="wfusion-rule-editor__result-toolbar">
                <div className="mode-toggle">
                  <button
                    type="button"
                    className={`toggle-btn ${resultViewMode === 'table' ? 'is-active' : ''}`}
                    onClick={() => setResultViewMode('table')}
                  >
                    {t('simulateDebug.parseResult.tableMode')}
                  </button>
                  <button
                    type="button"
                    className={`toggle-btn ${resultViewMode === 'json' ? 'is-active' : ''}`}
                    onClick={() => setResultViewMode('json')}
                  >
                    {t('simulateDebug.parseResult.jsonMode')}
                  </button>
                </div>
                {resultSummary ? (
                  <div className="wfusion-rule-editor__inline-stats wfusion-rule-editor__inline-stats--header">
                    <span className="wfusion-rule-editor__inline-stat">
                      {t('wfusionRuleEditor.summaryEvents')}
                      <strong>{resultSummary.event_count}</strong>
                    </span>
                    <span className="wfusion-rule-editor__inline-stat">
                      {t('wfusionRuleEditor.summaryMatches')}
                      <strong>{resultSummary.match_count}</strong>
                    </span>
                  </div>
                ) : null}
                <label className="switch">
                  <input
                    type="checkbox"
                    checked={showEmpty}
                    onChange={(e) => setShowEmpty(e.target.checked)}
                  />
                  <span className="switch-slider"></span>
                  <span className="switch-label">
                    {t('simulateDebug.parseResult.showEmpty')}
                  </span>
                </label>
              </div>
            </div>
            <div className="wfusion-rule-editor__result-body">
              <WfusionResultContent
                t={t}
                result={result}
                viewMode={resultViewMode}
                showEmpty={showEmpty}
              />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function WfusionRuleEditorPage() {
  return (
    <section className="page-panels">
      <article className="panel is-visible">
        <section className="panel-body">
          <WfusionRuleEditorContent />
        </section>
      </article>
    </section>
  );
}

export default WfusionRuleEditorPage;
