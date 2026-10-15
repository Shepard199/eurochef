import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
public class MessageBoxInputContradictionEvidence extends GhidraScript {
  DecompInterface d=new DecompInterface();
  void dec(long x)throws Exception{
    Function f=getFunctionContaining(toAddr(x));
    println(String.format("\n=== DEC 0x%08X ===",x));
    if(f==null){println("NO_FUNCTION");return;}
    println("FUNCTION="+f.getName()+" ENTRY="+f.getEntryPoint());
    DecompileResults r=d.decompileFunction(f,60,monitor);
    if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
  }
  public void run()throws Exception{
    d.openProgram(currentProgram);
    long[] a={0x004a6ae0L,0x004486a0L,0x00508508L,0x00508519L,0x00507e11L};
    for(long x:a)dec(x);
    d.dispose();
  }
}