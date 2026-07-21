import React, { startTransition, useDeferredValue, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { UpOutlined } from '@ant-design/icons';
import { useTranslation } from 'react-i18next';
import { useNavigate, useParams } from 'react-router-dom';
import { FloatButton, Modal } from 'antd';
import { useSystem } from '@/contexts/SystemContext';
import { fetchReleaseDetail, fetchReleaseDiff, rollbackRelease } from '@/services/release';
import DiffViewer from '@/components/diff/DiffViewer';
import { parseDiffText } from '@/components/diff/diffUtils';

const DIFF_BATCH_SIZE = 10;
const RELEASE_DETAIL_COLLAPSED_LINE_THRESHOLD = 100;

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

function adaptDiffFiles(files = [], startIndex = 0) {
  return files.map((file, index) => {
    const parsedFiles = file.diff_text ? parseDiffText(file.diff_text) : [];
    return {
      release_group: file.release_group || 'draft',
      file_path: file.file_path,
      old_path: file.old_path,
      change_type: file.change_type || 'modify',
      diff_text: file.diff_text,
      parsedDiff: parsedFiles?.[0] || null,
      diff_anchor_id: buildDiffAnchorId(
        file.release_group || 'draft',
        file.file_path,
        startIndex + index,
      ),
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

function buildDiffOverviewBucketsFromStats(files = []) {
  return files.reduce((buckets, file) => {
    const key = normalizeChangeBucket(file.change_type);
    buckets[key] += 1;
    return buckets;
  }, { modified: 0, deleted: 0, added: 0 });
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
  const { currentSystem, decoratePath } = useSystem();
  const [loading, setLoading] = useState(false);
  const [detail, setDetail] = useState(null);
  const [error, setError] = useState(null);
  const [focusedDiffAnchorId, setFocusedDiffAnchorId] = useState('');
  const [diffState, setDiffState] = useState({
    groups: [],
    files: [],
    stats: null,
    totalFiles: 0,
    hasMore: false,
    loading: false,
    loadingMore: false,
    initialized: false,
    error: null,
  });
  const loadMoreSentinelRef = useRef(null);
  const diffRequestPendingRef = useRef(false);
  const appendScrollRestoreRef = useRef(null);
  const deferredDiffFiles = useDeferredValue(diffState.files);

  const loadDetail = async () => {
    setLoading(true);
    setError(null);
    try {
      const response = await fetchReleaseDetail(releaseId, currentSystem);
      setDetail(response);
    } catch (err) {
      setError({
        message: err.message || t('systemRelease.detailLoadFailed'),
      });
    } finally {
      setLoading(false);
    }
  };

  const loadDiffPage = async ({ append, offset }) => {
    if (diffRequestPendingRef.current) {
      return;
    }

    if (append && typeof window !== 'undefined') {
      appendScrollRestoreRef.current = {
        left: window.scrollX,
        top: window.scrollY,
      };
    }

    diffRequestPendingRef.current = true;
    setDiffState((prev) => ({
      ...prev,
      loading: append ? prev.loading : true,
      loadingMore: append,
      error: append ? prev.error : null,
    }));

    try {
      const response = await fetchReleaseDiff(
        releaseId,
        {
          offset,
          limit: DIFF_BATCH_SIZE,
        },
        currentSystem,
      );
      const nextFiles = adaptDiffFiles(response?.files || [], offset);

      startTransition(() => {
        setDiffState((prev) => ({
          groups: Array.isArray(response?.groups) ? response.groups : prev.groups,
          files: append ? [...prev.files, ...nextFiles] : nextFiles,
          stats: response?.stats || prev.stats,
          totalFiles: response?.total_files ?? prev.totalFiles,
          hasMore: Boolean(response?.has_more),
          loading: false,
          loadingMore: false,
          initialized: true,
          error: null,
        }));
      });
    } catch (diffError) {
      setDiffState((prev) => ({
        ...prev,
        loading: false,
        loadingMore: false,
        initialized: true,
        error: {
          message: diffError.message || t('systemRelease.diffLoadFailed'),
        },
      }));
    } finally {
      diffRequestPendingRef.current = false;
    }
  };

  useLayoutEffect(() => {
    if (diffState.loadingMore || !appendScrollRestoreRef.current || typeof window === 'undefined') {
      return;
    }

    const { left, top } = appendScrollRestoreRef.current;
    appendScrollRestoreRef.current = null;

    const restoreScroll = () => {
      window.scrollTo(left, top);
    };

    restoreScroll();
    const frameId = window.requestAnimationFrame(restoreScroll);
    return () => window.cancelAnimationFrame(frameId);
  }, [diffState.files.length, diffState.loadingMore]);

  const handleDeviceRollback = async (deviceId, targetId) => {
    if (!window.confirm(t('systemRelease.rollbackConfirmMessage'))) {
      return;
    }

    try {
      setLoading(true);
      const result = await rollbackRelease(
        releaseId,
        [deviceId],
        targetId ? [targetId] : [],
        currentSystem,
      );
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
  }, [currentSystem, releaseId]);

  useEffect(() => {
    setDiffState({
      groups: [],
      files: [],
      stats: null,
      totalFiles: 0,
      hasMore: false,
      loading: false,
      loadingMore: false,
      initialized: false,
      error: null,
    });
    loadDiffPage({ append: false, offset: 0 });
  }, [currentSystem, releaseId]);

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

  const diffGroups = useMemo(() => {
    const loadedByGroup = diffState.files.reduce((accumulator, file) => {
      const key = file.release_group || 'draft';
      if (!accumulator[key]) {
        accumulator[key] = [];
      }
      accumulator[key].push(file);
      return accumulator;
    }, {});

    return Array.isArray(diffState.groups)
      ? diffState.groups.map((group) => ({
          ...group,
          loadedFiles: loadedByGroup[group.release_group] || [],
          loadedCount: (loadedByGroup[group.release_group] || []).length,
          changeBuckets: buildDiffOverviewBucketsFromStats(loadedByGroup[group.release_group] || []),
        }))
      : [];
  }, [diffState.files, diffState.groups]);

  useEffect(() => {
    if (!loadMoreSentinelRef.current || !diffState.hasMore || diffState.loading || diffState.loadingMore) {
      return undefined;
    }

    const observer = new IntersectionObserver(
      (entries) => {
        const [entry] = entries;
        if (!entry?.isIntersecting || diffRequestPendingRef.current) {
          return;
        }
        loadDiffPage({ append: true, offset: diffState.files.length });
      },
      {
        rootMargin: '240px 0px',
      },
    );

    observer.observe(loadMoreSentinelRef.current);
    return () => observer.disconnect();
  }, [diffState.files.length, diffState.hasMore, diffState.loading, diffState.loadingMore]);

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
          <button
            type="button"
            className="btn ghost"
            onClick={() => navigate(decoratePath('/system-release'))}
          >
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
  const totalDiffStats = diffState.stats || { files_changed: 0, insertions: 0, deletions: 0 };
  const loadedDiffFileCount = diffState.files.length;
  const totalDiffFileCount = diffState.totalFiles || 0;
  const hasDiffFiles = totalDiffFileCount > 0;

  return (
    <div className="panel is-visible">
      <div className="release-detail">
        <header className="release-detail-header">
          <div style={{ display: 'flex', gap: '12px', alignItems: 'center' }}>
          <button
            type="button"
            className="btn ghost"
            onClick={() => navigate(decoratePath('/system-release'))}
          >
            {t('systemRelease.backToList')}
          </button>
            <button
              type="button"
              className="btn"
              onClick={() => navigate(decoratePath(`/system-release/${releaseId}/prepublish`))}
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
            <span className="summary-label">{t('systemRelease.system')}</span>
            <span className="summary-value">
              {t(`navigation.system.${detail.system || currentSystem}`)}
            </span>
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
              <div className="release-diff-hint">
                {t('systemRelease.diffLoadedProgress', {
                  loaded: loadedDiffFileCount,
                  total: totalDiffFileCount,
                })}
              </div>
            </div>
            <div className="release-diff-totals">
              <span className="release-diff-total-chip">
                {t('systemRelease.changedFilesSummary')}: {totalDiffStats.files_changed}
              </span>
              <span className="release-diff-total-chip is-add">
                +{totalDiffStats.insertions}
              </span>
              <span className="release-diff-total-chip is-delete">
                -{totalDiffStats.deletions}
              </span>
            </div>
          </header>
          <div className="release-diff-shell">
            <aside className="release-diff-sidebar">
              <div className="release-diff-sidebar-summary">
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

                  return (
                    <section key={group.release_group} className="release-diff-sidebar-group">
                      <header className="release-diff-sidebar-group-header">
                        <div>
                          <h5>{title}</h5>
                          <p>{subtitle}</p>
                        </div>
                        <span className="release-diff-sidebar-group-count">
                          {group.total_files}
                        </span>
                      </header>
                      <div className="release-diff-sidebar-group-stats">
                        <span>M {group.changeBuckets.modified}</span>
                        <span>D {group.changeBuckets.deleted}</span>
                        <span>A {group.changeBuckets.added}</span>
                        <span>{t('systemRelease.diffFileCount', { count: group.total_files })}</span>
                      </div>
                      <div className="release-diff-sidebar-files">
                        {group.loadedFiles.map((file) => {
                          const codeStyle = getFileChangeCodeStyle(file.change_type);
                          return (
                            <button
                              key={file.diff_anchor_id}
                              type="button"
                              className="release-diff-file-item"
                              onClick={() => handleJumpToDiff(file.diff_anchor_id)}
                              title={t('systemRelease.jumpToDiff')}
                            >
                              <span className="release-diff-file-item-path">{file.file_path}</span>
                              <span
                                className="release-diff-file-item-code"
                                style={{
                                  borderColor: codeStyle.borderColor,
                                  background: codeStyle.background,
                                  color: codeStyle.color,
                                }}
                              >
                                {getFileChangeCode(file.change_type)}
                              </span>
                            </button>
                          );
                        })}
                        {!group.loadedFiles.length && group.total_files === 0 ? (
                          <div className="release-diff-file-item-empty">
                            {t('systemRelease.noFileChangesInCategory')}
                          </div>
                        ) : null}
                      </div>
                    </section>
                  );
                })}
              </div>
            </aside>

            <section className="release-diff-content-panel">
              {diffState.loading ? (
                <div className="release-diff-placeholder">{t('common.loading')}</div>
              ) : null}

              {!diffState.loading && diffState.error && !deferredDiffFiles.length ? (
                <div className="release-error-content">{diffState.error.message}</div>
              ) : null}

              {!diffState.loading && diffState.initialized && !hasDiffFiles ? (
                <div className="release-diff-placeholder">{t('systemRelease.noFileChangesInCategory')}</div>
              ) : null}

              {deferredDiffFiles.length > 0 ? (
                <DiffViewer
                  files={deferredDiffFiles}
                  viewType="split"
                  loading={false}
                  collapsedLineThreshold={RELEASE_DETAIL_COLLAPSED_LINE_THRESHOLD}
                  getFileAnchorId={(file) => file.diff_anchor_id}
                  focusedFileAnchorId={focusedDiffAnchorId}
                />
              ) : null}

              {diffState.error && deferredDiffFiles.length > 0 ? (
                <div className="release-error-content">{diffState.error.message}</div>
              ) : null}

              <div ref={loadMoreSentinelRef} className="release-diff-loader">
                {diffState.loadingMore ? t('systemRelease.diffLoadingMore') : null}
                {!diffState.hasMore && hasDiffFiles && loadedDiffFileCount >= totalDiffFileCount
                  ? t('systemRelease.diffLoadedAll')
                  : null}
              </div>
            </section>
          </div>
        </div>
      </div>
      <FloatButton.BackTop
        visibilityHeight={320}
        icon={<UpOutlined />}
        style={{ right: 24, bottom: 24 }}
      />
    </div>
  );
}

export default ReleaseDetailPage;
