import ghidra.app.decompiler.*;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.Function;
public class InputBindingInitEvidence extends GhidraScript {
  DecompInterface d=new DecompInterface();
  void dec(long x)throws Exception{
    Function f=getFunctionContaining(toAddr(x));
    println(String.format("\n=== 0x%08X ===",x));
    if(f==null){println("NO_FUNCTION");return;}
    println("FUNCTION="+f.getName()+" ENTRY="+f.getEntryPoint());
    DecompileResults r=d.decompileFunction(f,60,monitor);
    if(r.decompileCompleted()&&r.getDecompiledFunction()!=null)println(r.getDecompiledFunction().getC());
    else println("ERR="+r.getErrorMessage());
  }
  public void run()throws Exception{
    d.openProgram(currentProgram);
    long[] a={0x0050795dL,0x00508608L,0x00507e11L,0x00508508L,0x00509396L,0x005092b4L,0x00508c76L};
    for(long x:a)dec(x);
    d.dispose();
  }
}