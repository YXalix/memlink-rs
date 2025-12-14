use std::{
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};


use anyhow::{Context, Result, anyhow, bail};
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum ThreadPoolError {
    #[error("Thread pool size must be greater than 0")]
    InvalidSize,
    #[error("Failed to submit task: {0}")]
    TaskSubmissionFailed(String),
    #[error("Thread pool has been shutdown")]
    PoolShutdown,
    #[error("Failed to create thread: {0}")]
    ThreadCreationFailed(String),
    #[error("Failed to join thread: {0}")]
    ThreadJoinFailed(String),
    #[error("Worker thread panicked: {0}")]
    WorkerPanicked(String),
    #[error("Mutex lock poisoned: {0}")]
    MutexPoisoned(String),
    #[error("Job execution failed: {0}")]
    JobExecutionFailed(String),
}

type Job = Box<dyn FnOnce() + Send + 'static>;

#[derive(Debug)]
pub struct ThreadPool {
    workers: Vec<Worker>,
    sender: Option<mpsc::Sender<Job>>,
    is_shutdown: bool,
}

#[derive(Debug)]
struct Worker {
    id: usize,
    thread: thread::JoinHandle<()>,
}

impl Worker {
    fn new(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) -> Result<Worker, ThreadPoolError> {

        let thread_builder = thread::Builder::new();
        let thread = thread_builder
            .name(format!("worker:{}", id))
            .spawn(move || {
                Self::run_worker(id, receiver)
            }).map_err(|e| ThreadPoolError::ThreadCreationFailed(e.to_string()))?;

        Ok(Worker { id, thread })
    }

    fn run_worker(id: usize, receiver: Arc<Mutex<mpsc::Receiver<Job>>>) {
        loop {
            let job_result = || -> Result<(), anyhow::Error> {
                let lock_result = receiver.lock()
                    .map_err(|e| anyhow!("Failed to lock mutex: {}", e))?;
                let job = lock_result.recv_timeout(Duration::from_millis(100))
                    .context("Failed to receive job")?;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    job();
                }));
                if let Err(e) = result {
                    eprintln!("Worker {} panicked while executing a job", id);
                    if let Some(s) = e.downcast_ref::<String>() {
                        eprintln!("Panic message: {}", s);
                    } else if let Some(s) = e.downcast_ref::<&str>() {
                        eprintln!("Panic message: {}", s);
                    }
                }
                Ok(())
            }();

            match job_result {
                Ok(_) => continue,
                Err(e) => {
                    // 检查错误类型
                    if e.to_string().contains("disconnected") || 
                       e.to_string().contains("Failed to receive job") {
                        println!("Worker {} disconnected; shutting down.", id);
                        break;
                    } else if e.to_string().contains("poisoned") {
                        eprintln!("Worker {} detected a poisoned mutex, attempting to recover", id);
                        // 继续尝试
                        continue;
                    } else {
                        eprintln!("Worker {} encountered error: {}", id, e);
                        // 其他错误也继续尝试
                        continue;
                    }
                }
            }
        }
    }
}


impl ThreadPool {
    pub fn new(size: usize) -> Result<ThreadPool> {
        if size == 0 {
            bail!(ThreadPoolError::InvalidSize);
        }
        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));
        let mut workers = Vec::with_capacity(size);
        for id in 0..size {
            match Worker::new(id, Arc::clone(&receiver)) {
                Ok(worker) => workers.push(worker),
                Err(e) => {
                    // 如果第一个worker就失败，直接返回错误
                    if workers.is_empty() {
                        return Err(anyhow!("Failed to create first worker: {}", e));
                    }
                    // 否则记录警告并继续
                    eprintln!("Warning: Failed to create worker {}: {}", id, e);
                }
            }
        }
        // 确保至少创建了一个worker
        if workers.is_empty() {
            bail!("Failed to create any workers");
        }
        Ok(ThreadPool { 
            workers, 
            sender: Some(sender),
            is_shutdown: false,
        })
    }
    /// 获取线程池大小
    pub fn size(&self) -> usize {
        self.workers.len()
    }
    /// 检查线程池是否已关闭
    pub fn is_shutdown(&self) -> bool {
        self.is_shutdown
    }
    /// 执行任务
    pub fn execute<F>(&self, f: F) -> Result<()>
    where
        F: FnOnce() + Send + 'static,
    {
        if self.is_shutdown {
            bail!(ThreadPoolError::PoolShutdown);
        }
        
        let job = Box::new(f);

        self.sender
            .as_ref()
            .ok_or_else(|| anyhow!(ThreadPoolError::PoolShutdown))?
            .send(job)
            .map_err(|e| anyhow!(ThreadPoolError::TaskSubmissionFailed(e.to_string())))?;
            
        Ok(())
    }

    pub fn shutdown(&mut self) -> Result<()> {
        if self.is_shutdown {
            return Ok(());
        }
        
        self.is_shutdown = true;
        
        // 丢弃发送者，这样workers会在处理完所有任务后退出
        drop(self.sender.take());
        
        // 收集所有join错误
        let mut errors = Vec::new();
        
        for worker in self.workers.drain(..) {
            match worker.thread.join() {
                Ok(()) => (),
                Err(e) => {
                    let error_msg = if let Some(s) = e.downcast_ref::<String>() {
                        s.clone()
                    } else if let Some(s) = e.downcast_ref::<&str>() {
                        s.to_string()
                    } else {
                        "Unknown panic payload".to_string()
                    };
                    
                    errors.push(anyhow!(ThreadPoolError::WorkerPanicked(error_msg)));
                }
            }
        }
        
        if !errors.is_empty() {
            for error in &errors[1..] {
                eprintln!("Additional error during shutdown: {}", error);
            }
            return Err(anyhow!(ThreadPoolError::ThreadJoinFailed(
                errors[0].to_string(),
            )));
        }
        
        Ok(())
    }

    pub fn execute_with_fallback<F, E>(&self, task: F, fallback: E) -> Result<()>
    where
        F: FnOnce() + Send + 'static,
        E: FnOnce(anyhow::Error),
    {
        match self.execute(task) {
            Ok(()) => Ok(()),
            Err(e) => {
                fallback(e);
                Err(anyhow!(ThreadPoolError::JobExecutionFailed(
                    "Task execution failed, fallback executed".to_string(),
                )))
            }
        }
    }

    pub fn execute_batch<F>(&self, tasks: Vec<F>) -> Vec<Result<()>>
    where
        F: FnOnce() + Send + 'static + Clone,
    {
        tasks.into_iter()
            .map(|task| self.execute(task))
            .collect()
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        if !self.is_shutdown {
            if let Err(e) = self.shutdown() {
                eprintln!("Error during thread pool drop: {}", e);
            }
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_thread_pool_creation() -> Result<()> {
        let pool = ThreadPool::new(4)?;
        assert_eq!(pool.size(), 4);
        assert!(!pool.is_shutdown());
        
        let result = ThreadPool::new(0);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Thread pool size must be greater than 0"));
        
        Ok(())
    }

    #[test]
    fn test_basic_execution() -> Result<()> {
        let pool = ThreadPool::new(2)?;
        let counter = Arc::new(AtomicUsize::new(0));
        
        for _ in 0..5 {
            let counter = Arc::clone(&counter);
            pool.execute(move || {
                counter.fetch_add(1, Ordering::SeqCst);
                thread::sleep(Duration::from_millis(10));
            })?;
        }
        
        thread::sleep(Duration::from_millis(100));
        assert_eq!(counter.load(Ordering::SeqCst), 5);
        
        Ok(())
    }

    #[test]
    fn test_execute_after_shutdown() -> Result<()> {
        let mut pool = ThreadPool::new(2)?;
        
        pool.shutdown()?;
        assert!(pool.is_shutdown());
        
        let result = pool.execute(|| println!("This should fail"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("shutdown"));
        
        Ok(())
    }

    #[test]
    fn test_graceful_shutdown() -> Result<()> {
        let pool = ThreadPool::new(2)?;
        let (tx, rx) = mpsc::channel();
        let completed = Arc::new(AtomicUsize::new(0));
        
        for i in 0..3 {
            let tx = tx.clone();
            let completed = Arc::clone(&completed);
            pool.execute(move || {
                thread::sleep(Duration::from_millis(50));
                completed.fetch_add(1, Ordering::SeqCst);
                tx.send(i).unwrap();
            })?;
        }
        
        drop(pool); // 触发关闭
        
        let mut results = Vec::new();
        for _ in 0..3 {
            results.push(rx.recv()?);
        }
        
        results.sort();
        assert_eq!(results, vec![0, 1, 2]);
        Ok(())
    }

    #[test]
    fn test_execute_with_fallback() -> Result<()> {
        let pool = ThreadPool::new(2)?;
        let fallback_called = Arc::new(AtomicUsize::new(0));
        
        // 正常执行
        let fallback_called_clone = Arc::clone(&fallback_called);
        pool.execute_with_fallback(
            || println!("Task executed"),
            move |_| {
                fallback_called_clone.fetch_add(1, Ordering::SeqCst);
            }
        )?;
        
        assert_eq!(fallback_called.load(Ordering::SeqCst), 0);
        
        // 测试关闭后的回退
        let mut pool = ThreadPool::new(1)?;
        pool.shutdown()?;
        
        let fallback_called = Arc::new(AtomicUsize::new(0));
        let fallback_called_clone = Arc::clone(&fallback_called);
        
        let result = pool.execute_with_fallback(
            || {},
            move |_| {
                fallback_called_clone.fetch_add(1, Ordering::SeqCst);
            }
        );
        
        assert!(result.is_err());
        assert_eq!(fallback_called.load(Ordering::SeqCst), 1);
        
        Ok(())
    }

    #[test]
    fn test_panic_handling() -> Result<()> {
        let pool = ThreadPool::new(2)?;
        let (tx, rx) = mpsc::channel();
        
        // 第一个任务会panic
        pool.execute(|| {
            panic!("Intentional panic");
        })?;
        
        // 第二个任务应该仍然能执行
        pool.execute(move || {
            tx.send(42).unwrap();
        })?;
        
        thread::sleep(Duration::from_millis(100));
        
        assert_eq!(rx.try_recv()?, 42);
        Ok(())
    }

    #[test]
    fn test_concurrent_execution() -> Result<()> {
        let pool = Arc::new(ThreadPool::new(4)?);
        let counter = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();
        
        for _ in 0..10 {
            let counter = Arc::clone(&counter);
            let pool = Arc::clone(&pool);
            let handle = thread::spawn(move || {
                pool.execute(move || {
                    counter.fetch_add(1, Ordering::SeqCst);
                }).unwrap();
            });
            handles.push(handle);
        }
        
        for handle in handles {
            handle.join().unwrap();
        }
        
        thread::sleep(Duration::from_millis(100));
        assert_eq!(counter.load(Ordering::SeqCst), 10);
        
        Ok(())
    }
    
    #[test]
    fn test_execute_batch() -> Result<()> {
        let pool = ThreadPool::new(3)?;
        let counter = Arc::new(AtomicUsize::new(0));
        
        let tasks: Vec<_> = (0..6)
            .map(|_| {
                let counter = Arc::clone(&counter);
                move || {
                    counter.fetch_add(1, Ordering::SeqCst);
                    thread::sleep(Duration::from_millis(10));
                }
            })
            .collect();
        
        let results = pool.execute_batch(tasks);
        
        // 所有任务都应该成功提交
        assert_eq!(results.len(), 6);
        assert!(results.iter().all(|r| r.is_ok()));
        
        thread::sleep(Duration::from_millis(200));
        assert_eq!(counter.load(Ordering::SeqCst), 6);
        
        Ok(())
    }
    
    #[test]
    fn test_thread_names() -> Result<()> {
        let pool = ThreadPool::new(2)?;
        
        // 验证线程名可以通过系统工具查看
        pool.execute(|| {
            let binding = thread::current();
            let name = binding.name().unwrap_or("unnamed");
            println!("Thread name: {}", name);
            assert!(name.starts_with("worker-"));
        })?;
        
        thread::sleep(Duration::from_millis(50));
        Ok(())
    }
}