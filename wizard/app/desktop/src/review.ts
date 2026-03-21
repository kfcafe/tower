/**
 * Review and artifact inspection UI components for Wizard
 */

import type {
  Artifact,
  ArtifactType, 
  Review,
  ReviewType,
  ReviewDecision,
  ReviewStatus,
  VerificationDetails,
  VerificationStatus,
  VerificationCheck,
  VerificationIssue,
  IssueSeverity
} from './types';

export interface ReviewPanelProps {
  unitId?: string;
  onReviewComplete?: (reviewId: string, decision: ReviewDecision, notes?: string) => void;
}

export interface ArtifactViewerProps {
  artifacts: Artifact[];
  onArtifactSelect?: (artifact: Artifact) => void;
  onRequestReview?: (artifactId: string) => void;
}

export interface VerificationPanelProps {
  unitId: string;
  verificationResults?: VerificationDetails[];
  onRunVerification?: (unitId: string) => void;
}

/**
 * Main review panel component for inspecting unit work
 */
export function ReviewPanel({ unitId, onReviewComplete }: ReviewPanelProps) {
  return {
    type: 'review-panel',
    unitId,
    sections: [
      {
        id: 'artifacts',
        title: 'Generated Artifacts',
        collapsible: true,
        defaultExpanded: true,
        content: {
          type: 'artifact-list',
          filter: unitId ? { unitId } : undefined,
          showActions: true,
        }
      },
      {
        id: 'reviews',
        title: 'Review History', 
        collapsible: true,
        defaultExpanded: false,
        content: {
          type: 'review-history',
          filter: unitId ? { unitId } : undefined,
        }
      },
      {
        id: 'verification',
        title: 'Verification Results',
        collapsible: true,
        defaultExpanded: true,
        content: {
          type: 'verification-results',
          unitId,
          showRunButton: true,
        }
      },
      {
        id: 'actions',
        title: 'Review Actions',
        collapsible: false,
        content: {
          type: 'review-actions',
          unitId,
          onReviewComplete,
        }
      }
    ]
  };
}

/**
 * Artifact viewer component for browsing generated artifacts
 */
export function ArtifactViewer({ artifacts, onArtifactSelect, onRequestReview }: ArtifactViewerProps) {
  const groupedArtifacts = groupArtifactsByType(artifacts);
  
  return {
    type: 'artifact-viewer',
    layout: 'grid',
    groups: Object.entries(groupedArtifacts).map(([type, artifacts]) => ({
      title: formatArtifactTypeName(type),
      artifacts: artifacts.map(artifact => ({
        id: artifact.id,
        name: artifact.path.split('/').pop() || artifact.path,
        path: artifact.path,
        size: formatFileSize(artifact.size),
        created: formatTimestamp(artifact.created_at),
        reviewed: artifact.reviewed,
        reviewStatus: artifact.review_status,
        actions: [
          {
            label: 'View',
            action: () => onArtifactSelect?.(artifact),
          },
          {
            label: 'Review',
            action: () => onRequestReview?.(artifact.id),
            disabled: artifact.reviewed,
          }
        ]
      }))
    }))
  };
}

/**
 * Verification panel for unit validation
 */
export function VerificationPanel({ unitId, verificationResults, onRunVerification }: VerificationPanelProps) {
  const latestResult = verificationResults?.[verificationResults.length - 1];
  
  return {
    type: 'verification-panel',
    unitId,
    header: {
      title: 'Unit Verification',
      actions: [
        {
          label: 'Run Verification',
          action: () => onRunVerification?.(unitId),
          variant: 'primary',
        }
      ]
    },
    content: latestResult ? {
      type: 'verification-result',
      summary: latestResult.summary,
      overallStatus: determineOverallStatus(latestResult),
      sections: [
        {
          title: 'Verification Checks',
          content: latestResult.checks.map(formatVerificationCheck),
        },
        {
          title: 'Issues Found',
          content: latestResult.issues.map(formatVerificationIssue),
          collapsible: true,
          defaultExpanded: latestResult.issues.length > 0,
        },
        {
          title: 'Verified Artifacts',
          content: latestResult.verified_artifacts,
          collapsible: true,
          defaultExpanded: false,
        }
      ]
    } : {
      type: 'verification-placeholder',
      message: 'No verification results available. Click "Run Verification" to validate this unit.',
    }
  };
}

/**
 * Review decision form component
 */
export function ReviewDecisionForm({ 
  reviewId, 
  onSubmit 
}: { 
  reviewId: string;
  onSubmit: (decision: ReviewDecision, notes?: string) => void;
}) {
  return {
    type: 'review-decision-form',
    reviewId,
    fields: [
      {
        name: 'decision',
        type: 'select',
        label: 'Review Decision',
        required: true,
        options: [
          { value: 'approve', label: 'Approve' },
          { value: 'request-changes', label: 'Request Changes' },
          { value: 'reject', label: 'Reject' },
          { value: 'needs-info', label: 'Needs More Information' },
          { value: 'defer', label: 'Defer Decision' },
        ]
      },
      {
        name: 'notes',
        type: 'textarea',
        label: 'Review Notes',
        placeholder: 'Add any comments or feedback...',
        rows: 4,
      },
      {
        name: 'required_changes',
        type: 'text-array',
        label: 'Required Changes',
        placeholder: 'Describe what needs to be changed...',
        condition: { field: 'decision', value: 'request-changes' }
      },
      {
        name: 'questions',
        type: 'text-array', 
        label: 'Questions',
        placeholder: 'What information is needed?',
        condition: { field: 'decision', value: 'needs-info' }
      },
      {
        name: 'reason',
        type: 'textarea',
        label: 'Rejection Reason',
        placeholder: 'Explain why this work is being rejected...',
        required: true,
        condition: { field: 'decision', value: 'reject' }
      }
    ],
    actions: [
      {
        label: 'Cancel',
        variant: 'secondary',
      },
      {
        label: 'Submit Review',
        variant: 'primary',
        type: 'submit',
      }
    ],
    onSubmit: (formData: any) => {
      const decision = buildReviewDecision(formData);
      onSubmit(decision, formData.notes);
    }
  };
}

// Helper functions

function groupArtifactsByType(artifacts: Artifact[]): Record<string, Artifact[]> {
  return artifacts.reduce((groups, artifact) => {
    const type = getArtifactTypeKey(artifact.artifact_type);
    if (!groups[type]) {
      groups[type] = [];
    }
    groups[type].push(artifact);
    return groups;
  }, {} as Record<string, Artifact[]>);
}

function getArtifactTypeKey(artifactType: ArtifactType): string {
  if (typeof artifactType === 'object') {
    return Object.keys(artifactType)[0];
  }
  return artifactType;
}

function formatArtifactTypeName(type: string): string {
  return type.split('-').map(word => 
    word.charAt(0).toUpperCase() + word.slice(1)
  ).join(' ');
}

function formatFileSize(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
}

function formatTimestamp(timestamp: string | number): string {
  const date = new Date(timestamp);
  return date.toLocaleDateString() + ' ' + date.toLocaleTimeString();
}

function determineOverallStatus(result: VerificationDetails): VerificationStatus {
  if (result.checks.some(check => check.status === 'Failed')) {
    return 'Failed';
  }
  if (result.issues.some(issue => issue.severity === 'Error' || issue.severity === 'Critical')) {
    return 'Failed';
  }
  if (result.checks.some(check => check.status === 'InProgress')) {
    return 'InProgress';
  }
  return 'Passed';
}

function formatVerificationCheck(check: VerificationCheck) {
  return {
    name: check.name,
    status: check.status,
    description: check.description,
    duration: `${check.duration}ms`,
    output: check.output,
    statusIcon: getStatusIcon(check.status),
    statusColor: getStatusColor(check.status),
  };
}

function formatVerificationIssue(issue: VerificationIssue) {
  return {
    severity: issue.severity,
    message: issue.message,
    file: issue.file,
    line: issue.line,
    suggestion: issue.suggestion,
    severityIcon: getSeverityIcon(issue.severity),
    severityColor: getSeverityColor(issue.severity),
  };
}

function getStatusIcon(status: VerificationStatus): string {
  switch (status) {
    case 'Passed': return '✅';
    case 'Failed': return '❌';
    case 'InProgress': return '🔄';
    case 'Skipped': return '⏭️';
    case 'Error': return '⚠️';
    default: return '❓';
  }
}

function getStatusColor(status: VerificationStatus): string {
  switch (status) {
    case 'Passed': return 'green';
    case 'Failed': return 'red';
    case 'InProgress': return 'blue';
    case 'Skipped': return 'gray';
    case 'Error': return 'orange';
    default: return 'gray';
  }
}

function getSeverityIcon(severity: IssueSeverity): string {
  switch (severity) {
    case 'Info': return 'ℹ️';
    case 'Warning': return '⚠️';
    case 'Error': return '❌';
    case 'Critical': return '🚨';
    default: return '❓';
  }
}

function getSeverityColor(severity: IssueSeverity): string {
  switch (severity) {
    case 'Info': return 'blue';
    case 'Warning': return 'yellow';
    case 'Error': return 'red';
    case 'Critical': return 'darkred';
    default: return 'gray';
  }
}

function buildReviewDecision(formData: any): ReviewDecision {
  switch (formData.decision) {
    case 'approve':
      return { type: 'Approve' };
    case 'request-changes':
      return { 
        type: 'RequestChanges', 
        required_changes: formData.required_changes || [] 
      };
    case 'reject':
      return { 
        type: 'Reject', 
        reason: formData.reason || 'No reason provided' 
      };
    case 'needs-info':
      return { 
        type: 'NeedsInfo', 
        questions: formData.questions || [] 
      };
    case 'defer':
      return { 
        type: 'Defer', 
        until: formData.until ? new Date(formData.until) : undefined 
      };
    default:
      throw new Error(`Unknown review decision: ${formData.decision}`);
  }
}

/**
 * Review workflow hooks for managing review state
 */
export function useReviewWorkflow(unitId?: string) {
  return {
    // Load artifacts for the unit
    async loadArtifacts() {
      // Would call into Rust backend
      return [];
    },
    
    // Load review history
    async loadReviewHistory() {
      // Would call into Rust backend  
      return [];
    },
    
    // Load verification results
    async loadVerificationResults() {
      // Would call into Rust backend
      return [];
    },
    
    // Request a new review
    async requestReview(reviewType: ReviewType) {
      // Would call into Rust backend
      return 'review-id';
    },
    
    // Complete a review
    async completeReview(reviewId: string, decision: ReviewDecision, notes?: string) {
      // Would call into Rust backend
    },
    
    // Run unit verification
    async runVerification() {
      // Would call into Rust backend
      return 'verification-id';
    }
  };
}