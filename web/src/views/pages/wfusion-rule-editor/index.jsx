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

const SAMPLE_WFS = `window conn_events {
    stream = "netflow"
    time = event_time
    over = 30m
    fields {
        sip: ip
        dip: ip
        dport: digit
        bytes_out: digit
        protocol: chars
        event_time: time
    }
}

window auth_events {
    stream = "auth_events"
    time = event_time
    over = 30m
    fields {
        sip: ip
        dip: ip
        dport: digit
        service: chars
        user: chars
        result: chars
        event_time: time
    }
}

window security_alerts {
    over = 0
    fields {
        sip: ip
        dip: ip
        alert_type: chars
        detail: chars
    }
}`;

const SAMPLE_WFL = `rule rat_propagation {
    events {
        scan  : conn_events && (dport == 22 || dport == 445 || dport == 3389) && bytes_out < 1000
        login : auth_events && result == "success"
        xfer  : conn_events && bytes_out >= 10000
    }
    match<sip,dip:5m> {
        on event {
            scan | count >= 1;
            login | count >= 1;
            xfer | count >= 1;
        }
    } -> score(95.0)
    entity(ip, scan.sip)
    yield security_alerts (
        sip = scan.sip,
        dip = scan.dip,
        alert_type = "rat_propagation",
        detail = "scan -> login -> xfer"
    )
    limits { max_memory = "64MB"; max_instances = 10000; on_exceed = throttle; }
}`;

const SAMPLE_EVENTS = `{"_stream":"netflow","sip":"10.0.0.99","dip":"192.168.1.10","dport":22,"bytes_out":100,"protocol":"tcp","event_time":1700000000000000000}
{"_stream":"auth_events","sip":"10.0.0.99","dip":"192.168.1.10","dport":22,"service":"ssh","user":"root","result":"success","event_time":1700000001000000000}
{"_stream":"netflow","sip":"10.0.0.99","dip":"192.168.1.10","dport":22,"bytes_out":50000,"protocol":"tcp","event_time":1700000002000000000}`;

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
