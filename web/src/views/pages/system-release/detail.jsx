import React, { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate, useParams } from 'react-router-dom';
import { Modal } from 'antd';
import { fetchReleaseDetail, rollbackRelease } from '@/services/release';
import DiffViewer from '@/components/diff/DiffViewer';
import { parseDiffText } from '@/components/diff/diffUtils';

function sanitizeAnchorSegment(value = '') {
  return String(value)
    .trim()
    .replace(/[^a-zA-Z0-9/_-]+/g, '-')
    .replace(/[\\/]+/g, '-')
    .replace(/-+/g, '-')
    .replace(/^-|-$/g, '')
    .toLowerCase();
}

function buildDiffAnchorId(releaseGroup, filePath, index) {
  return `release-diff-${sanitizeAnchorSegment(releaseGroup)}-${index}-${sanitizeAnchorSegment(filePath)}`;
}

function adaptDiffFiles(files = [], releaseGroup = 'draft') {
  return files.map((file, index) => {
    const parsedFiles = file.diff_text ? parseDiffText(file.diff_text) : [];
    return {
      file_path: file.file_path,
      old_path: file.old_path,
      change_type: file.change_type || 'modify',
      diff_text: file.diff_text,
      parsedDiff: parsedFiles?.[0] || null,
      diff_anchor_id: buildDiffAnchorId(releaseGroup, file.file_path, index),
    };
  });
}

function normalizeChangeBucket(changeType) {
  if (changeType === 'add') {
    return 'added';
  }
  if (changeType === 'delete') {
    return 'deleted';
  }
  return 'modified';
}

function buildDiffOverviewBuckets(files = []) {
  return files.reduce(
    (buckets, file) => {
      const key = normalizeChangeBucket(file.change_type);
      buckets[key].push(file);
      return buckets;
    },
    { modified: [], deleted: [], added: [] },
  );
}

function getFileChangeCode(changeType) {
  if (changeType === 'add') {
    return 'A';
  }
  if (changeType === 'delete') {
    return 'D';
  }
  return 'M';
}

function getFileChangeCodeStyle(changeType) {
  if (changeType === 'add') {
    return {
      color: '#17b26a',
      background: 'rgba(23, 178, 106, 0.12)',
      borderColor: 'rgba(23, 178, 106, 0.24)',
    };
  }
  if (changeType === 'delete') {
    return {
      color: '#f1554c',
      background: 'rgba(241, 85, 76, 0.12)',
      borderColor: 'rgba(241, 85, 76, 0.24)',
    };
  }
  return {
    color: '#f79009',
    background: 'rgba(247, 144, 9, 0.12)',
    borderColor: 'rgba(247, 144, 9, 0.24)',
  };
}

function ReleaseDetailPage() {
  const { t } = useTranslation();
  const { id: releaseId } = useParams();
  const navigate = useNavigate();
  const [loading, setLoading] = useState(false);
  const [detail, setDetail] = useState(null);
  const [error, setError] = useState(null);
  const [focusedDiffAnchorId, setFocusedDiffAnchorId] = useState('');

  const loadDetail = async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await fetchReleaseDetail(releaseId);
      setDetail(response);
    } catch (err) {
      setError({
        message: err.message || t('systemRelease.detailLoadFailed'),
      });
    } finally {
      setLoading(false);
    }
  };

  const handleDeviceRollback = async (deviceId, targetId) => {
    if (!window.confirm(t('systemRelease.rollbackConfirmMessage'))) {
      return;
    }

    try {
      setLoading(true);
      const result = await rollbackRelease(releaseId, [deviceId], targetId ? [targetId] : []);
      Modal.success({
        title: t('systemRelease.rollbackSuccess'),
        content: result.message || t('systemRelease.rollbackSuccessMessage'),
      });
      await loadDetail();
    } catch (rollbackError) {
      Modal.error({
        title: t('systemRelease.rollbackFailed'),
        content: rollbackError.message || t('systemRelease.rollbackFailedMessage'),
      });
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadDetail();
  }, [releaseId]);

  const getReleaseGroupTitle = (releaseGroup) => {
    if (releaseGroup === 'models') {
      return t('systemRelease.groupModels');
    }
    if (releaseGroup === 'infra') {
      return t('systemRelease.groupInfra');
    }
    if (releaseGroup === 'all') {
      return t('systemRelease.groupAll');
    }
    return t('systemRelease.draftLabel');
  };

  const diffGroups = useMemo(
    () =>
      Array.isArray(detail?.diff_groups)
        ? detail.diff_groups.map((group) => ({
            ...group,
            adaptedFiles: adaptDiffFiles(group.files || [], group.release_group),
          }))
        : [],
    [detail],
  );

  const getReleaseStatusMeta = () => {
    const normalizedStatus = String(detail?.status || '').toUpperCase();
    const releaseGroup = detail?.release_group || 'draft';

    if (normalizedStatus === 'RUNNING') {
      return { className: 'is-running', text: t('systemRelease.statusRunning') };
    }
    if (normalizedStatus === 'FAIL' || normalizedStatus === 'PARTIAL_FAIL') {
      return { className: 'is-fail', text: t('systemRelease.statusFailed') };
    }
    if (normalizedStatus === 'PASS') {
      if (releaseGroup === 'models') {
        return { className: 'is-pass', text: t('systemRelease.statusPublishedModels') };
      }
      if (releaseGroup === 'infra') {
        return { className: 'is-pass', text: t('systemRelease.statusPublishedInfra') };
      }
      if (releaseGroup === 'all') {
        return { className: 'is-pass', text: t('systemRelease.statusPublishedAll') };
      }
    }
    return { className: 'is-wait', text: t('systemRelease.statusDraft') };
  };

  const translateStageLabel = (label) => {
    const stageMap = {
      沙盒: t('systemRelease.stageSandbox'),
      发布: t('systemRelease.stagePublish'),
      发布规则: t('systemRelease.stagePublishModels'),
      发布设施: t('systemRelease.stagePublishInfra'),
      发布全量: t('systemRelease.stagePublishAll'),
      准备: t('systemRelease.stagePrepare'),
      调用客户端: t('systemRelease.stageCallClient'),
      运行状态: t('systemRelease.stageRuntime'),
    };
    return stageMap[label] || label;
  };

  const renderStageChip = (stage, index) => {
    const status = String(stage?.status || '').toLowerCase();
    let className = 'stage-icon';
    let icon = '>>';
    if (status === 'pass') {
      className = 'stage-icon is-pass';
      icon = '✓';
    } else if (status === 'fail') {
      className = 'stage-icon is-fail';
      icon = '✗';
    } else if (status === 'running') {
      className = 'stage-icon is-running';
      icon = '…';
    }
    return (
      <span key={index} className="stage-chip">
        <span className={className} title={translateStageLabel(stage.label || '')}>
          {icon}
        </span>
        <span className="stage-name">{translateStageLabel(stage.label || '')}</span>
      </span>
    );
  };

  const handleJumpToDiff = (anchorId) => {
    if (!anchorId) {
      return;
    }

    setFocusedDiffAnchorId(anchorId);

    const target = document.getElementById(anchorId);
    if (!target) {
      return;
    }

    const nextUrl = `${window.location.pathname}${window.location.search}#${anchorId}`;
    window.history.replaceState(null, '', nextUrl);
    target.scrollIntoView({ behavior: 'smooth', block: 'start' });
  };

  if (loading) {
    return (
      <div className="panel is-visible">
        <div style={{ padding: '40px', textAlign: 'center' }}>{t('common.loading')}</div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="panel is-visible">
        <div style={{ padding: '40px', textAlign: 'center' }}>
          <div>{error.message}</div>
          <button type="button" className="btn ghost" onClick={() => navigate('/system-release')}>
            {t('systemRelease.backToList')}
          </button>
        </div>
      </div>
    );
  }

  if (!detail) {
    return (
      <div className="panel is-visible">
        <div style={{ padding: '40px', textAlign: 'center' }}>{t('common.noData')}</div>
      </div>
    );
  }

  const statusMeta = getReleaseStatusMeta();
  const isPublished = ['PASS', 'FAIL', 'PARTIAL_FAIL'].includes(String(detail.status || '').toUpperCase());
  const releaseGroupLabel =
    detail.release_group === 'models'
      ? t('systemRelease.groupModels')
      : detail.release_group === 'infra'
        ? t('systemRelease.groupInfra')
        : detail.release_group === 'all'
          ? t('systemRelease.groupAll')
          : t('systemRelease.draftLabel');

  return (
    <div className="panel is-visible">
      <div className="release-detail">
        <header className="release-detail-header">
          <div style={{ display: 'flex', gap: '12px', alignItems: 'center' }}>
            <button type="button" className="btn ghost" onClick={() => navigate('/system-release')}>
              {t('systemRelease.backToList')}
            </button>
            <button
              type="button"
              className="btn"
              onClick={() => navigate(`/system-release/${releaseId}/prepublish`)}
            >
              {t(isPublished ? 'sandbox.prepublishDetail' : 'sandbox.startSandbox')}
            </button>
          </div>
          <h3 id="detail-title">{t('systemRelease.versionDetail', { version: detail.version })}</h3>
        </header>

        <div className="release-summary">
          <div className="summary-item">
            <span className="summary-label">{t('systemRelease.status')}</span>
            <span className={`summary-value ${statusMeta.className}`}>{statusMeta.text}</span>
          </div>
          <div className="summary-item">
            <span className="summary-label">{t('systemRelease.version')}</span>
            <span className="summary-value">{detail.version || '—'}</span>
          </div>
          <div className="summary-item">
            <span className="summary-label">{t('systemRelease.releaseGroup')}</span>
            <span className="summary-value">{releaseGroupLabel}</span>
          </div>
          <div className="summary-item">
            <span className="summary-label">{t('systemRelease.remark')}</span>
            <span className="summary-value">{detail.pipeline || '—'}</span>
          </div>
          <div className="summary-item">
            <span className="summary-label">{t('systemRelease.stages')}</span>
            <div className="summary-stages">
              {(detail.stages || []).map(renderStageChip)}
            </div>
          </div>
        </div>

        {Array.isArray(detail.devices) && detail.devices.length > 0 ? (
          <div className="release-devices" style={{ margin: '20px 0' }}>
            <header className="release-diff-header">
              <h4>{t('systemRelease.devicesTitle')}</h4>
            </header>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '12px', marginTop: '12px' }}>
              {detail.devices.map((device) => {
                const deviceStatus = String(device.status || '').toUpperCase();
                const deviceClass =
                  deviceStatus === 'SUCCESS' || deviceStatus === 'ROLLED_BACK'
                    ? 'is-pass'
                    : deviceStatus === 'FAIL'
                      ? 'is-fail'
                      : deviceStatus === 'RUNNING' || deviceStatus === 'QUEUED' || deviceStatus === 'ROLLBACKING'
                        ? 'is-running'
                        : 'is-wait';
                const machineLabel = device.device_name
                  ? `${device.device_name} (${device.ip}:${device.port})`
                  : `${device.ip}:${device.port}`;

                return (
                  <div
                    key={device.id}
                    style={{
                      border: '1px solid #f0f0f0',
                      borderRadius: '10px',
                      padding: '12px 16px',
                      background: '#fafafa',
                    }}
                  >
                    <div style={{ display: 'flex', alignItems: 'center', gap: '10px', marginBottom: '10px' }}>
                      <span style={{ fontWeight: 600, fontSize: '14px' }}>{machineLabel}</span>
                      <span className={`release-status ${deviceClass}`}>{deviceStatus || '—'}</span>
                      {(deviceStatus === 'FAIL' || deviceStatus === 'SUCCESS') && (
                        <button
                          type="button"
                          className="btn ghost"
                          style={{ marginLeft: 'auto', fontSize: '12px', padding: '4px 12px' }}
                          onClick={() => handleDeviceRollback(device.device_id, device.id)}
                        >
                          {t('systemRelease.rollback')}
                        </button>
                      )}
                    </div>
                    {Array.isArray(device.stage_trace) && device.stage_trace.length > 0 ? (
                      <div className="summary-stages">{device.stage_trace.map(renderStageChip)}</div>
                    ) : null}
                    {device.error_message ? (
                      <div style={{ marginTop: '8px', fontSize: '13px', color: '#f1554c' }}>
                        {device.error_message}
                      </div>
                    ) : null}
                  </div>
                );
              })}
            </div>
          </div>
        ) : null}

        <div className="release-diff">
          <header className="release-diff-header">
            <div>
              <h4 style={{ marginBottom: 4 }}>
                {t(detail.status === 'WAIT' ? 'systemRelease.draftVersionDiff' : 'systemRelease.versionDiff')}
              </h4>
            </div>
          </header>
          <div style={{ marginTop: '24px', display: 'grid', gap: '20px' }}>
            {diffGroups.map((group) => {
              const title = getReleaseGroupTitle(group.release_group);
              const subtitle = group.previous_version
                ? t('systemRelease.diffGroupVersionVsPrevious', {
                    current: group.current_version,
                    previous: group.previous_version,
                  })
                : t('systemRelease.diffGroupVersionVsInitial', {
                    current: group.current_version === 'draft'
                      ? t('systemRelease.draftLabel')
                      : group.current_version,
                  });
              const overviewBuckets = buildDiffOverviewBuckets(group.adaptedFiles);
              const changeStats = [
                {
                  key: 'modified',
                  label: `M ${t('systemRelease.changedFilesSummary')}`,
                  count: overviewBuckets.modified.length,
                  color: '#f79009',
                  background: 'rgba(247, 144, 9, 0.08)',
                },
                {
                  key: 'deleted',
                  label: `D ${t('systemRelease.deletedFilesSummary')}`,
                  count: overviewBuckets.deleted.length,
                  color: '#f1554c',
                  background: 'rgba(241, 85, 76, 0.08)',
                },
                {
                  key: 'added',
                  label: `A ${t('systemRelease.addedFilesSummary')}`,
                  count: overviewBuckets.added.length,
                  color: '#17b26a',
                  background: 'rgba(23, 178, 106, 0.08)',
                },
              ];

              return (
                <section
                  key={group.release_group}
                  style={{
                    border: '1px solid #eceff5',
                    borderRadius: '12px',
                    padding: '16px',
                    background: '#fff',
                  }}
                >
                  <header style={{ marginBottom: '12px' }}>
                    <h4 style={{ margin: 0 }}>{title}</h4>
                    <div style={{ marginTop: '4px', fontSize: '12px', color: '#667085' }}>
                      {subtitle}
                    </div>
                  </header>
                  <section
                    style={{
                      marginBottom: '16px',
                      padding: '14px',
                      border: '1px solid #e5e7eb',
                      borderRadius: '12px',
                      background: '#f8fafc',
                    }}
                  >
                    <div
                      style={{
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'space-between',
                        gap: '12px',
                        marginBottom: '10px',
                        flexWrap: 'wrap',
                      }}
                    >
                      <div style={{ fontSize: '13px', fontWeight: 600, color: '#344054' }}>
                        {t('systemRelease.fileChangeSummary')}
                      </div>
                      <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
                        {changeStats.map((item) => (
                          <span
                            key={item.key}
                            style={{
                              display: 'inline-flex',
                              alignItems: 'center',
                              gap: '6px',
                              padding: '4px 10px',
                              borderRadius: '999px',
                              background: item.background,
                              color: item.color,
                              fontSize: '12px',
                              fontWeight: 700,
                            }}
                          >
                            <span>{item.label}</span>
                            <span>{item.count}</span>
                          </span>
                        ))}
                      </div>
                    </div>
                    <div
                      style={{
                        display: 'grid',
                        gridTemplateColumns: 'repeat(3, minmax(0, 1fr))',
                        gap: '12px',
                        alignItems: 'stretch',
                      }}
                    >
                      {group.adaptedFiles.length > 0 ? (
                        group.adaptedFiles.map((file) => {
                          const codeStyle = getFileChangeCodeStyle(file.change_type);
                          return (
                            <button
                              key={file.diff_anchor_id}
                              type="button"
                              onClick={() => handleJumpToDiff(file.diff_anchor_id)}
                              style={{
                                width: '100%',
                                minHeight: '56px',
                                display: 'flex',
                                alignItems: 'center',
                                justifyContent: 'space-between',
                                gap: '12px',
                                padding: '12px 14px',
                                border: '1px solid #e5e7eb',
                                borderRadius: '12px',
                                background: '#fff',
                                color: '#1f2937',
                                textAlign: 'left',
                                cursor: 'pointer',
                              }}
                              title={t('systemRelease.jumpToDiff')}
                            >
                              <span
                                style={{
                                  flex: 1,
                                  minWidth: 0,
                                  fontFamily:
                                    "'SFMono-Regular', Consolas, 'Liberation Mono', Menlo, ui-monospace, monospace",
                                  fontSize: '12px',
                                  lineHeight: 1.5,
                                  overflowWrap: 'anywhere',
                                }}
                              >
                                {file.file_path}
                              </span>
                              <span
                                style={{
                                  flexShrink: 0,
                                  minWidth: '28px',
                                  padding: '4px 8px',
                                  border: `1px solid ${codeStyle.borderColor}`,
                                  borderRadius: '999px',
                                  background: codeStyle.background,
                                  color: codeStyle.color,
                                  fontSize: '12px',
                                  fontWeight: 800,
                                  textAlign: 'center',
                                }}
                              >
                                {getFileChangeCode(file.change_type)}
                              </span>
                            </button>
                          );
                        })
                      ) : (
                        <div style={{ padding: '14px', fontSize: '12px', color: '#98a2b3' }}>
                          {t('systemRelease.noFileChangesInCategory')}
                        </div>
                      )}
                    </div>
                  </section>
                  <DiffViewer
                    files={group.adaptedFiles}
                    viewType="split"
                    loading={false}
                    getFileAnchorId={(file) => file.diff_anchor_id}
                    focusedFileAnchorId={focusedDiffAnchorId}
                  />
                </section>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
}

export default ReleaseDetailPage;
