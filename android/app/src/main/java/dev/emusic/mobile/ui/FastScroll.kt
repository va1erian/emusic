package dev.emusic.mobile.ui

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectVerticalDragGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.grid.LazyGridState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch

/** The scroll position of a lazy list or grid, as a fast-scroller sees it. */
interface ScrollAdapter {
    val totalItems: Int
    val firstVisible: Int
    val visibleItems: Int
    val isScrolling: Boolean
    suspend fun scrollTo(index: Int)
}

@Composable
fun rememberListScrollAdapter(state: LazyListState): ScrollAdapter = remember(state) {
    object : ScrollAdapter {
        override val totalItems get() = state.layoutInfo.totalItemsCount
        override val firstVisible get() = state.firstVisibleItemIndex
        override val visibleItems get() = state.layoutInfo.visibleItemsInfo.size
        override val isScrolling get() = state.isScrollInProgress
        override suspend fun scrollTo(index: Int) = state.scrollToItem(index)
    }
}

@Composable
fun rememberGridScrollAdapter(state: LazyGridState): ScrollAdapter = remember(state) {
    object : ScrollAdapter {
        override val totalItems get() = state.layoutInfo.totalItemsCount
        override val firstVisible get() = state.firstVisibleItemIndex
        override val visibleItems get() = state.layoutInfo.visibleItemsInfo.size
        override val isScrolling get() = state.isScrollInProgress
        override suspend fun scrollTo(index: Int) = state.scrollToItem(index)
    }
}

/** Lists shorter than this many screens scroll fine without a thumb. */
private const val MIN_SCREENS = 4

private val THUMB_HEIGHT = 56.dp
private val TOUCH_WIDTH = 28.dp

/**
 * Wraps a lazy list or grid with a draggable thumb on its trailing edge, so a
 * library of thousands of rows can be crossed in one drag. The thumb shows only
 * for long content and brightens while scrolling or dragging; only the thumb
 * takes touches, so rows underneath the edge stay tappable.
 */
@Composable
fun FastScrollBox(
    state: ScrollAdapter,
    modifier: Modifier = Modifier,
    content: @Composable BoxScope.() -> Unit,
) {
    BoxWithConstraints(modifier = modifier) {
        content()
        val total = state.totalItems
        val visible = state.visibleItems
        if (visible == 0 || total < visible * MIN_SCREENS) return@BoxWithConstraints

        val scope = rememberCoroutineScope()
        var dragging by remember { mutableStateOf(false) }
        val density = LocalDensity.current
        val trackPx = with(density) { (maxHeight - THUMB_HEIGHT).toPx() }.coerceAtLeast(1f)
        val scrollable = (total - visible).coerceAtLeast(1)
        val fraction = (state.firstVisible.toFloat() / scrollable).coerceIn(0f, 1f)
        val alpha by animateFloatAsState(
            targetValue = if (dragging || state.isScrolling) 1f else 0.45f,
            label = "fast-scroll-alpha",
        )

        // While dragging, the thumb follows the finger (not the list), and
        // the list is scrolled to match.
        var dragY by remember { mutableStateOf(0f) }
        val latestFraction by rememberUpdatedState(fraction)
        val thumbY = if (dragging) dragY else fraction * trackPx

        Box(
            modifier = Modifier
                .align(Alignment.TopEnd)
                .offset(y = with(density) { thumbY.toDp() })
                .size(width = TOUCH_WIDTH, height = THUMB_HEIGHT)
                .testTag("fast_scroll")
                .pointerInput(scrollable, trackPx) {
                    detectVerticalDragGestures(
                        onDragStart = {
                            dragY = latestFraction * trackPx
                            dragging = true
                        },
                        onDragEnd = { dragging = false },
                        onDragCancel = { dragging = false },
                        onVerticalDrag = { change, delta ->
                            change.consume()
                            dragY = (dragY + delta).coerceIn(0f, trackPx)
                            val index = (dragY / trackPx * scrollable).toInt()
                            scope.launch { state.scrollTo(index) }
                        },
                    )
                },
        ) {
            Box(
                modifier = Modifier
                    .align(Alignment.CenterEnd)
                    .size(width = if (dragging) 8.dp else 5.dp, height = THUMB_HEIGHT)
                    .alpha(alpha)
                    .background(MaterialTheme.colorScheme.primary, RoundedCornerShape(4.dp)),
            )
        }
    }
}
