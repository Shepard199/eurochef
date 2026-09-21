import ghidra.app.script.GhidraScript;
import ghidra.program.model.listing.*;
public class PiranhaStack24Evidence extends GhidraScript {
  @Override public void run()throws Exception{
    disassemble(toAddr(0x004676F0L));
    Function f=getFunctionAt(toAddr(0x004676F0L));
    if(f==null){try{f=createFunction(toAddr(0x004676F0L),"evidence_4676f0");}catch(Exception ignored){}}
    InstructionIterator it=currentProgram.getListing().getInstructions(f.getBody(),true);
    while(it.hasNext()){Instruction ins=it.next();String s=ins.toString();if(s.contains("ESP + 0x24")||s.contains("[ESI + 0x4]")||s.contains("+ 0x150")||s.contains("+ 0x154"))println(ins.getAddress()+"  "+s);}
  }
}